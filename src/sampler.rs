use crate::collectors::{disk, docker, net, process, system};
use crate::config::Config;
use crate::state::{DiskIo, Net, SharedState, Snapshot};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sysinfo::{
    CpuRefreshKind, Disks, MemoryRefreshKind, Networks, ProcessRefreshKind, ProcessesToUpdate,
    RefreshKind, System,
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

    // CPU% needs two refreshes spaced apart; warm up so the first
    // snapshot already has real numbers.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL.max(Duration::from_millis(250)));

    let mut prev = Prev {
        at: Instant::now(),
        net: to_map(net::counters(&networks)),
        disk: to_map(disk::io_counters().unwrap_or_default()),
        docker_cpu: docker::PrevCpu::new(),
    };

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
                if docker_is_up { "connected" } else { "unavailable" }
            );
            docker_was_up = docker_is_up;
        }

        let snapshot = Snapshot {
            ts: SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0),
            interval_secs: config.interval.as_secs_f64(),
            host: system::host(&sys),
            cpu: system::cpu(&sys),
            memory: system::memory(&sys),
            disks: disk::usage(&disks),
            disk_io,
            net: net_rates,
            processes: process::top(&sys, config.top_n),
            docker: docker_stats,
        };

        let docker_cpu = std::mem::take(&mut prev.docker_cpu);
        prev = Prev {
            at: sampled_at,
            net: to_map(net_counters),
            disk: to_map(disk_counters.unwrap_or_default()),
            docker_cpu,
        };

        if let Ok(json) = serde_json::to_string(&snapshot) {
            match state.lock() {
                Ok(mut s) => *s = json,
                Err(poisoned) => *poisoned.into_inner() = json,
            }
        }

        work = tick_start.elapsed();
    }
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
