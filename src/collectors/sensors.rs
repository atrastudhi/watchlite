use crate::state::{Fan, Sensors, Temp};
use sysinfo::Components;

/// Temperatures via sysinfo (cross-platform), fans via /sys/class/hwmon
/// (Linux only). None when the host exposes no sensors (typical for VMs)
/// so the dashboard hides the panel.
pub fn read(components: &Components) -> Option<Sensors> {
    let mut temps: Vec<Temp> = components
        .iter()
        .filter_map(|c| {
            c.temperature().map(|t| Temp {
                label: c.label().to_string(),
                temp_c: t,
                critical_c: c.critical(),
            })
        })
        .filter(|t| t.temp_c > 0.0)
        .collect();
    temps.sort_by(|a, b| b.temp_c.total_cmp(&a.temp_c));
    temps.truncate(16);

    let fans = read_fans();

    if temps.is_empty() && fans.is_empty() {
        None
    } else {
        Some(Sensors { temps, fans })
    }
}

#[cfg(target_os = "linux")]
fn read_fans() -> Vec<Fan> {
    let mut fans = Vec::new();
    let Ok(hwmons) = std::fs::read_dir("/sys/class/hwmon") else {
        return fans;
    };
    for hwmon in hwmons.flatten() {
        let dir = hwmon.path();
        let chip = std::fs::read_to_string(dir.join("name"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        for i in 1..=8 {
            let Ok(raw) = std::fs::read_to_string(dir.join(format!("fan{i}_input"))) else {
                continue;
            };
            let Ok(rpm) = raw.trim().parse::<u64>() else {
                continue;
            };
            let label = std::fs::read_to_string(dir.join(format!("fan{i}_label")))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| format!("{chip} fan{i}"));
            fans.push(Fan { label, rpm });
        }
    }
    fans.truncate(8);
    fans
}

#[cfg(not(target_os = "linux"))]
fn read_fans() -> Vec<Fan> {
    Vec::new()
}
