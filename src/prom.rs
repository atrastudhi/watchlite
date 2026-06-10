//! Prometheus text exposition format, rendered once per sampling tick.

use crate::state::Snapshot;
use std::fmt::Write;

pub fn render(s: &Snapshot) -> String {
    let mut o = String::with_capacity(4096);

    gauge(
        &mut o,
        "watchlite_uptime_seconds",
        s.host.uptime_secs as f64,
    );
    gauge(&mut o, "watchlite_cpu_pct", s.cpu.total_pct as f64);
    head(&mut o, "watchlite_cpu_core_pct", "gauge");
    for (i, pct) in s.cpu.per_core_pct.iter().enumerate() {
        let _ = writeln!(o, "watchlite_cpu_core_pct{{core=\"{i}\"}} {pct}");
    }
    gauge(&mut o, "watchlite_load1", s.cpu.load_avg[0]);
    gauge(&mut o, "watchlite_load5", s.cpu.load_avg[1]);
    gauge(&mut o, "watchlite_load15", s.cpu.load_avg[2]);

    gauge(&mut o, "watchlite_mem_total_bytes", s.memory.total as f64);
    gauge(&mut o, "watchlite_mem_used_bytes", s.memory.used as f64);
    gauge(
        &mut o,
        "watchlite_mem_available_bytes",
        s.memory.available as f64,
    );
    gauge(
        &mut o,
        "watchlite_swap_total_bytes",
        s.memory.swap_total as f64,
    );
    gauge(
        &mut o,
        "watchlite_swap_used_bytes",
        s.memory.swap_used as f64,
    );

    head(&mut o, "watchlite_disk_total_bytes", "gauge");
    for d in &s.disks {
        let _ = writeln!(
            o,
            "watchlite_disk_total_bytes{{mount=\"{}\"}} {}",
            esc(&d.mount),
            d.total
        );
    }
    head(&mut o, "watchlite_disk_used_bytes", "gauge");
    for d in &s.disks {
        let _ = writeln!(
            o,
            "watchlite_disk_used_bytes{{mount=\"{}\"}} {}",
            esc(&d.mount),
            d.used
        );
    }
    if let Some(io) = &s.disk_io {
        head(&mut o, "watchlite_disk_read_bytes_per_sec", "gauge");
        for d in io {
            let _ = writeln!(
                o,
                "watchlite_disk_read_bytes_per_sec{{device=\"{}\"}} {}",
                esc(&d.device),
                d.read_bps
            );
        }
        head(&mut o, "watchlite_disk_write_bytes_per_sec", "gauge");
        for d in io {
            let _ = writeln!(
                o,
                "watchlite_disk_write_bytes_per_sec{{device=\"{}\"}} {}",
                esc(&d.device),
                d.write_bps
            );
        }
    }

    head(&mut o, "watchlite_net_rx_bytes_per_sec", "gauge");
    for n in &s.net {
        let _ = writeln!(
            o,
            "watchlite_net_rx_bytes_per_sec{{iface=\"{}\"}} {}",
            esc(&n.iface),
            n.rx_bps
        );
    }
    head(&mut o, "watchlite_net_tx_bytes_per_sec", "gauge");
    for n in &s.net {
        let _ = writeln!(
            o,
            "watchlite_net_tx_bytes_per_sec{{iface=\"{}\"}} {}",
            esc(&n.iface),
            n.tx_bps
        );
    }
    head(&mut o, "watchlite_net_rx_bytes_total", "counter");
    for n in &s.net {
        let _ = writeln!(
            o,
            "watchlite_net_rx_bytes_total{{iface=\"{}\"}} {}",
            esc(&n.iface),
            n.rx_total
        );
    }
    head(&mut o, "watchlite_net_tx_bytes_total", "counter");
    for n in &s.net {
        let _ = writeln!(
            o,
            "watchlite_net_tx_bytes_total{{iface=\"{}\"}} {}",
            esc(&n.iface),
            n.tx_total
        );
    }

    gauge(&mut o, "watchlite_procs_total", s.processes.total as f64);
    gauge(
        &mut o,
        "watchlite_procs_running",
        s.processes.running as f64,
    );
    gauge(
        &mut o,
        "watchlite_procs_sleeping",
        s.processes.sleeping as f64,
    );
    gauge(&mut o, "watchlite_procs_zombie", s.processes.zombie as f64);

    if let Some(c) = &s.connections {
        gauge(&mut o, "watchlite_tcp_established", c.established as f64);
        gauge(&mut o, "watchlite_tcp_time_wait", c.time_wait as f64);
        gauge(
            &mut o,
            "watchlite_tcp_listening_ports",
            c.listening.len() as f64,
        );
    }

    if let Some(d) = &s.docker {
        head(&mut o, "watchlite_container_cpu_pct", "gauge");
        for c in &d.containers {
            let _ = writeln!(
                o,
                "watchlite_container_cpu_pct{{name=\"{}\"}} {}",
                esc(&c.name),
                c.cpu_pct
            );
        }
        head(&mut o, "watchlite_container_mem_bytes", "gauge");
        for c in &d.containers {
            let _ = writeln!(
                o,
                "watchlite_container_mem_bytes{{name=\"{}\"}} {}",
                esc(&c.name),
                c.mem_bytes
            );
        }
    }

    if let Some(sens) = &s.sensors {
        head(&mut o, "watchlite_temp_celsius", "gauge");
        for t in &sens.temps {
            let _ = writeln!(
                o,
                "watchlite_temp_celsius{{sensor=\"{}\"}} {}",
                esc(&t.label),
                t.temp_c
            );
        }
        if !sens.fans.is_empty() {
            head(&mut o, "watchlite_fan_rpm", "gauge");
            for f in &sens.fans {
                let _ = writeln!(
                    o,
                    "watchlite_fan_rpm{{fan=\"{}\"}} {}",
                    esc(&f.label),
                    f.rpm
                );
            }
        }
    }

    gauge(&mut o, "watchlite_alerts_firing", s.alerts.len() as f64);

    o
}

fn head(o: &mut String, name: &str, kind: &str) {
    let _ = writeln!(o, "# TYPE {name} {kind}");
}

fn gauge(o: &mut String, name: &str, value: f64) {
    head(o, name, "gauge");
    let _ = writeln!(o, "{name} {value}");
}

fn esc(v: &str) -> String {
    v.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::esc;

    #[test]
    fn escapes_label_values() {
        assert_eq!(esc("plain"), "plain");
        assert_eq!(esc("with \"quotes\""), "with \\\"quotes\\\"");
        assert_eq!(esc("back\\slash"), "back\\\\slash");
        assert_eq!(esc("new\nline"), "new\\nline");
    }
}
