//! Threshold alerts with hysteresis. Webhook delivery shells out to curl so
//! https works without bundling a TLS stack; without curl, events still log
//! to stderr.

use crate::config::{AlertSpec, Config};
use crate::state::{AlertStatus, Snapshot};

/// Consecutive ticks a condition must hold before firing/resolving,
/// so a single spike doesn't flap the alert.
const HOLD_TICKS: u32 = 3;

struct RuleState {
    spec: AlertSpec,
    over: u32,
    under: u32,
    firing_since: Option<u64>,
}

pub struct AlertEngine {
    rules: Vec<RuleState>,
    webhook: Option<String>,
    hostname: String,
}

impl AlertEngine {
    pub fn new(config: &Config) -> AlertEngine {
        AlertEngine {
            rules: config
                .alerts
                .iter()
                .map(|spec| RuleState {
                    spec: spec.clone(),
                    over: 0,
                    under: 0,
                    firing_since: None,
                })
                .collect(),
            webhook: config.webhook.clone(),
            hostname: String::new(),
        }
    }

    /// Evaluate all rules against this tick's snapshot; returns currently
    /// firing alerts for embedding in the JSON snapshot.
    pub fn eval(&mut self, snap: &Snapshot) -> Vec<AlertStatus> {
        self.hostname = snap.host.hostname.clone();
        let mut firing = Vec::new();
        for rule in &mut self.rules {
            let (raw, label, unit) = measure(&rule.spec, snap);
            let value = (raw * 10.0).round() / 10.0;

            if value > rule.spec.threshold {
                rule.over += 1;
                rule.under = 0;
            } else {
                rule.under += 1;
                rule.over = 0;
            }

            match rule.firing_since {
                None if rule.over >= HOLD_TICKS => {
                    rule.firing_since = Some(snap.ts);
                    emit(
                        &self.hostname,
                        &label,
                        value,
                        rule.spec.threshold,
                        unit,
                        "firing",
                        self.webhook.as_deref(),
                    );
                }
                Some(_) if rule.under >= HOLD_TICKS => {
                    rule.firing_since = None;
                    emit(
                        &self.hostname,
                        &label,
                        value,
                        rule.spec.threshold,
                        unit,
                        "resolved",
                        self.webhook.as_deref(),
                    );
                }
                _ => {}
            }

            if let Some(since) = rule.firing_since {
                firing.push(AlertStatus {
                    metric: label,
                    value,
                    threshold: rule.spec.threshold,
                    unit: unit.to_string(),
                    since,
                });
            }
        }
        firing
    }
}

/// Compute the current value, display label, and unit for a rule. `disk` and
/// `temp` optionally scope to a target (mount path / sensor-label substring);
/// without one they take the max across all disks / sensors.
fn measure(spec: &AlertSpec, snap: &Snapshot) -> (f64, String, &'static str) {
    let label = match &spec.target {
        Some(t) => format!("{}:{t}", spec.metric),
        None => spec.metric.clone(),
    };
    let value = match spec.metric.as_str() {
        "cpu" => snap.cpu.total_pct as f64,
        "mem" => pct(snap.memory.used, snap.memory.total),
        "swap" => pct(snap.memory.swap_used, snap.memory.swap_total),
        "disk" => snap
            .disks
            .iter()
            .filter(|d| spec.target.as_deref().is_none_or(|t| d.mount == t))
            .map(|d| pct(d.used, d.total))
            .fold(0.0, f64::max),
        "temp" => snap
            .sensors
            .as_ref()
            .map(|s| {
                s.temps
                    .iter()
                    .filter(|t| spec.target.as_deref().is_none_or(|q| t.label.contains(q)))
                    .map(|t| t.temp_c as f64)
                    .fold(0.0, f64::max)
            })
            .unwrap_or(0.0),
        _ => 0.0,
    };
    let unit = if spec.metric == "temp" {
        "\u{b0}C"
    } else {
        "%"
    };
    (value, label, unit)
}

fn pct(used: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        used as f64 / total as f64 * 100.0
    }
}

#[allow(clippy::too_many_arguments)]
fn emit(
    host: &str,
    label: &str,
    value: f64,
    threshold: f64,
    unit: &str,
    event: &str,
    webhook: Option<&str>,
) {
    eprintln!("alert {event}: {label} {value}{unit} (threshold {threshold}{unit})");
    let Some(url) = webhook else { return };
    let payload = payload_for(url, host, label, value, threshold, unit, event);
    let spawned = std::process::Command::new("curl")
        .args([
            "-fsS",
            "-m",
            "5",
            "-X",
            "POST",
            "-H",
            "Content-Type: application/json",
            "-d",
            &payload,
        ])
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match spawned {
        // Reap in a detached thread so we never block the sampler or leak zombies.
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(e) => eprintln!("alert webhook failed (is curl installed?): {e}"),
    }
}

/// Discord webhook URLs are auto-detected and get Discord's `content`
/// payload shape; everything else gets the generic JSON event.
fn payload_for(
    url: &str,
    host: &str,
    label: &str,
    value: f64,
    threshold: f64,
    unit: &str,
    event: &str,
) -> String {
    let host = host.replace(['"', '\\'], "");
    let label = label.replace(['"', '\\'], "");
    if url.contains("discord.com/api/webhooks/") || url.contains("discordapp.com/api/webhooks/") {
        let emoji = if event == "firing" {
            "\u{1F534}"
        } else {
            "\u{1F7E2}"
        };
        return format!(
            "{{\"content\":\"{emoji} **{label}** on **{host}**: {value}{unit} (threshold {threshold}{unit}) \u{2014} {event}\"}}"
        );
    }
    format!(
        "{{\"host\":\"{host}\",\"metric\":\"{label}\",\"value\":{value},\"threshold\":{threshold},\"unit\":\"{unit}\",\"state\":\"{event}\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::*;

    fn snapshot(cpu_pct: f32) -> Snapshot {
        Snapshot {
            ts: 1000,
            interval_secs: 2.0,
            host: Host {
                hostname: "test".into(),
                os: "test".into(),
                kernel: "test".into(),
                arch: "test".into(),
                uptime_secs: 1,
                cpu_count: 1,
                cpu_model: "test".into(),
                cpu_freq_mhz: 0,
            },
            cpu: Cpu {
                total_pct: cpu_pct,
                per_core_pct: vec![],
                load_avg: [0.0; 3],
            },
            memory: Memory {
                total: 100,
                used: 50,
                available: 50,
                swap_total: 0,
                swap_used: 0,
            },
            disks: vec![],
            disk_io: None,
            net: vec![],
            processes: Processes {
                total: 0,
                running: 0,
                sleeping: 0,
                zombie: 0,
                list: vec![],
            },
            docker: None,
            docker_hint: None,
            sensors: None,
            connections: None,
            alerts: vec![],
        }
    }

    fn engine(threshold: f64) -> AlertEngine {
        AlertEngine {
            rules: vec![RuleState {
                spec: AlertSpec {
                    metric: "cpu".into(),
                    target: None,
                    threshold,
                },
                over: 0,
                under: 0,
                firing_since: None,
            }],
            webhook: None,
            hostname: String::new(),
        }
    }

    #[test]
    fn payload_shapes_by_destination() {
        let generic = payload_for(
            "https://ntfy.sh/topic",
            "web-01",
            "cpu",
            94.2,
            90.0,
            "%",
            "firing",
        );
        assert_eq!(
            generic,
            "{\"host\":\"web-01\",\"metric\":\"cpu\",\"value\":94.2,\"threshold\":90,\"unit\":\"%\",\"state\":\"firing\"}"
        );

        let discord = payload_for(
            "https://discord.com/api/webhooks/123/abc",
            "web-01",
            "cpu",
            94.2,
            90.0,
            "%",
            "firing",
        );
        assert!(discord.starts_with("{\"content\":\""), "{discord}");
        assert!(discord.contains("**cpu** on **web-01**: 94.2% (threshold 90%)"));

        // scoped disk rule with °C unit and a path in the label still serializes
        let temp = payload_for(
            "https://ntfy.sh/topic",
            "web-01",
            "temp:Package",
            81.0,
            80.0,
            "\u{b0}C",
            "firing",
        );
        // valid JSON in every shape
        for p in [&generic, &discord, &temp] {
            serde_json::from_str::<serde_json::Value>(p).unwrap();
        }
    }

    #[test]
    fn scoped_disk_targets_one_mount() {
        let mut snap = snapshot(0.0);
        snap.disks = vec![
            Disk {
                mount: "/".into(),
                fs: "ext4".into(),
                total: 100,
                used: 10,
            },
            Disk {
                mount: "/data".into(),
                fs: "ext4".into(),
                total: 100,
                used: 95,
            },
        ];
        let scoped = AlertSpec {
            metric: "disk".into(),
            target: Some("/data".into()),
            threshold: 90.0,
        };
        let (v, label, unit) = measure(&scoped, &snap);
        assert_eq!(v, 95.0);
        assert_eq!(label, "disk:/data");
        assert_eq!(unit, "%");

        // unscoped disk takes the busiest mount
        let any = AlertSpec {
            metric: "disk".into(),
            target: None,
            threshold: 90.0,
        };
        assert_eq!(measure(&any, &snap).0, 95.0);
    }

    #[test]
    fn fires_only_after_hold_ticks() {
        let mut e = engine(90.0);
        assert!(e.eval(&snapshot(95.0)).is_empty());
        assert!(e.eval(&snapshot(95.0)).is_empty());
        assert_eq!(
            e.eval(&snapshot(95.0)).len(),
            1,
            "fires on third consecutive tick"
        );
    }

    #[test]
    fn spike_does_not_fire() {
        let mut e = engine(90.0);
        assert!(e.eval(&snapshot(95.0)).is_empty());
        assert!(e.eval(&snapshot(10.0)).is_empty());
        assert!(e.eval(&snapshot(95.0)).is_empty());
        assert!(e.eval(&snapshot(10.0)).is_empty());
    }

    #[test]
    fn resolves_after_hold_ticks_below() {
        let mut e = engine(90.0);
        for _ in 0..3 {
            e.eval(&snapshot(95.0));
        }
        assert_eq!(
            e.eval(&snapshot(10.0)).len(),
            1,
            "still firing while under hold"
        );
        assert_eq!(e.eval(&snapshot(10.0)).len(), 1);
        assert!(
            e.eval(&snapshot(10.0)).is_empty(),
            "resolves on third tick below"
        );
    }
}
