use crate::alerts::AlertEngine;
use crate::collectors::{connections, disk, docker, net, process, sensors, system};
use crate::config::Config;
use crate::prom;
use crate::state::{DiskIo, HistPoint, Net, SharedState, Snapshot};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind,
    ProcessesToUpdate, RefreshKind, System,
};

/// Previous-tick cumulative counters. Rebuilt from current keys each tick so
/// interface/device/container churn can't grow these maps unboundedly.
struct Prev {
    at: Instant,
    net: HashMap<String, (u64, u64)>,
    disk: HashMap<String, (u64, u64)>,
    docker_cpu: docker::PrevCpu,
}

pub fn run(state: SharedState, config: Config) {
    let refresh = RefreshKind::nothing()
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage())
        .with_memory(MemoryRefreshKind::everything());
    let proc_refresh = ProcessRefreshKind::nothing().with_cpu().with_memory();

    let mut sys = System::new_with_specifics(refresh);
    let mut networks = Networks::new_with_refreshed_list();
    let mut disks = Disks::new_with_refreshed_list();
    let mut components = Components::new_with_refreshed_list();

    // CPU% needs two refreshes spaced apart; warm up so the first
    // snapshot already has real numbers.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250)));

    let mut prev = Prev {
        at: Instant::now(),
        net: to_map(net::counters(&networks)),
        disk: to_map(disk::io_counters().unwrap_or_default()),
        docker_cpu: docker::PrevCpu::new(),
    };

    let mut alert_engine = AlertEngine::new(&config);
    let history_cap =
        (config.history.as_secs_f64() / config.interval.as_secs_f64()).ceil() as usize;
    let mut docker_was_up = false;
    let mut work = Duration::ZERO;
    loop {
        // Subtract last tick's sampling cost so ticks stay near the interval.
        std::thread::sleep(config.interval.saturating_sub(work));
        let tick_start = Instant::now();

        sys.refresh_specifics(refresh);
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, proc_refresh);
        networks.refresh(true);
        disks.refresh(true);
        components.refresh(true);

        let elapsed = prev.at.elapsed().as_secs_f64();
        let sampled_at = Instant::now();

        let net_counters = net::counters(&networks);
        let net_rates: Vec<Net> = net_counters
            .iter()
            .map(|(name, rx, tx)| {
                let (prx, ptx) = prev.net.get(name).copied().unwrap_or((*rx, *tx));
                Net {
                    iface: name.clone(),
                    rx_bps: rate(*rx, prx, elapsed),
                    tx_bps: rate(*tx, ptx, elapsed),
                    rx_total: *rx,
                    tx_total: *tx,
                }
            })
            .collect();

        let disk_counters = disk::io_counters();
        let disk_io: Option<Vec<DiskIo>> = disk_counters.as_ref().map(|counters| {
            counters
                .iter()
                .map(|(name, rd, wr)| {
                    let (prd, pwr) = prev.disk.get(name).copied().unwrap_or((*rd, *wr));
                    DiskIo {
                        device: name.clone(),
                        read_bps: rate(*rd, prd, elapsed),
                        write_bps: rate(*wr, pwr, elapsed),
                    }
                })
                .collect()
        });

        let docker_stats = if config.docker {
            docker::collect(&mut prev.docker_cpu)
        } else {
            None
        };
        let docker_is_up = docker_stats.is_some();
        if config.docker && docker_is_up != docker_was_up {
            eprintln!(
                "docker collector: {}",
                if docker_is_up {
                    "connected"
                } else {
                    "unavailable"
                }
            );
            docker_was_up = docker_is_up;
        }

        let mut snapshot = Snapshot {
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            interval_secs: config.interval.as_secs_f64(),
            host: system::host(&sys),
            cpu: system::cpu(&sys),
            memory: system::memory(&sys),
            disks: disk::usage(&disks),
            disk_io,
            net: net_rates,
            processes: process::top(&sys, config.top_n),
            docker: docker_stats,
            sensors: sensors::read(&components),
            connections: connections::read(),
            alerts: Vec::new(),
        };
        snapshot.alerts = alert_engine.eval(&snapshot);

        let docker_cpu = std::mem::take(&mut prev.docker_cpu);
        prev = Prev {
            at: sampled_at,
            net: to_map(net_counters),
            disk: to_map(disk_counters.unwrap_or_default()),
            docker_cpu,
        };

        let point = HistPoint {
            ts: snapshot.ts,
            cpu: snapshot.cpu.total_pct,
            mem: if snapshot.memory.total == 0 {
                0.0
            } else {
                (snapshot.memory.used as f64 / snapshot.memory.total as f64 * 100.0) as f32
            },
            rx: snapshot.net.iter().map(|n| n.rx_bps).sum(),
            tx: snapshot.net.iter().map(|n| n.tx_bps).sum(),
        };

        let prom_text = prom::render(&snapshot);
        if let Ok(json) = serde_json::to_string(&snapshot) {
            *lock_or_recover(&state.json) = json;
        }
        *lock_or_recover(&state.prom) = prom_text;
        {
            let mut hist = lock_or_recover(&state.history);
            hist.push_back(point);
            while hist.len() > history_cap {
                hist.pop_front();
            }
        }

        work = tick_start.elapsed();
    }
}

/// A poisoned mutex here just means a panicked sampler tick mid-write;
/// the data is a plain String/VecDeque, safe to keep using.
fn lock_or_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn to_map(v: Vec<(String, u64, u64)>) -> HashMap<String, (u64, u64)> {
    v.into_iter().map(|(k, a, b)| (k, (a, b))).collect()
}

/// Bytes/sec from cumulative counters; divides by *measured* elapsed time and
/// saturates on counter resets (recreated iface, restarted container).
fn rate(cur: u64, prev: u64, elapsed: f64) -> u64 {
    if elapsed <= 0.0 {
        return 0;
    }
    (cur.saturating_sub(prev) as f64 / elapsed) as u64
}
