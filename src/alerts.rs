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
                .map(|spec| RuleState { spec: spec.clone(), over: 0, under: 0, firing_since: None })
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
            let value = match rule.spec.metric.as_str() {
                "cpu" => snap.cpu.total_pct as f64,
                "mem" => {
                    if snap.memory.total == 0 {
                        0.0
                    } else {
                        snap.memory.used as f64 / snap.memory.total as f64 * 100.0
                    }
                }
                "disk" => snap
                    .disks
                    .iter()
                    .map(|d| d.used as f64 / d.total as f64 * 100.0)
                    .fold(0.0, f64::max),
                _ => 0.0,
            };
            let value = (value * 10.0).round() / 10.0;

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
                    emit(&self.hostname, &rule.spec, value, "firing", self.webhook.as_deref());
                }
                Some(_) if rule.under >= HOLD_TICKS => {
                    rule.firing_since = None;
                    emit(&self.hostname, &rule.spec, value, "resolved", self.webhook.as_deref());
                }
                _ => {}
            }

            if let Some(since) = rule.firing_since {
                firing.push(AlertStatus {
                    metric: rule.spec.metric.clone(),
                    value,
                    threshold: rule.spec.threshold,
                    since,
                });
            }
        }
        firing
    }
}

fn emit(host: &str, spec: &AlertSpec, value: f64, event: &str, webhook: Option<&str>) {
    eprintln!(
        "alert {event}: {} {value}% (threshold {}%)",
        spec.metric, spec.threshold
    );
    let Some(url) = webhook else { return };
    let payload = format!(
        "{{\"host\":\"{}\",\"metric\":\"{}\",\"value\":{value},\"threshold\":{},\"state\":\"{event}\"}}",
        host.replace('"', ""),
        spec.metric,
        spec.threshold
    );
    let spawned = std::process::Command::new("curl")
        .args(["-fsS", "-m", "5", "-X", "POST", "-H", "Content-Type: application/json", "-d", &payload])
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
