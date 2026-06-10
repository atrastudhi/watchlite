use crate::state::Disk;
use std::collections::HashSet;
use sysinfo::Disks;

const PSEUDO_FS: &[&str] = &[
    "tmpfs", "devtmpfs", "overlay", "squashfs", "proc", "sysfs", "devfs", "autofs", "ramfs",
];

pub fn usage(disks: &Disks) -> Vec<Disk> {
    let mut seen = HashSet::new();
    disks
        .iter()
        .filter(|d| {
            let fs = d.file_system().to_string_lossy().to_lowercase();
            !PSEUDO_FS.contains(&fs.as_str())
        })
        .filter(|d| seen.insert(d.mount_point().to_path_buf()))
        .map(|d| Disk {
            mount: d.mount_point().to_string_lossy().into_owned(),
            fs: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
        })
        .filter(|d| d.total > 0)
        .collect()
}

/// Cumulative (read_bytes, written_bytes) per whole block device.
/// Linux only — parsed from /proc/diskstats. The sampler turns these into rates.
#[cfg(target_os = "linux")]
pub fn io_counters() -> Option<Vec<(String, u64, u64)>> {
    // /proc/diskstats sector counts are always in 512-byte units.
    const SECTOR: u64 = 512;
    let content = std::fs::read_to_string("/proc/diskstats").ok()?;
    let mut out = Vec::new();
    for line in content.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        if !is_whole_device(name) {
            continue;
        }
        let (Ok(rd), Ok(wr)) = (f[5].parse::<u64>(), f[9].parse::<u64>()) else {
            continue;
        };
        out.push((name.to_string(), rd * SECTOR, wr * SECTOR));
    }
    Some(out)
}

#[cfg(target_os = "linux")]
fn is_whole_device(name: &str) -> bool {
    // Whole devices (not partitions) appear in /sys/block.
    std::path::Path::new("/sys/block").join(name).exists() && !name.starts_with("loop") && !name.starts_with("ram")
}

#[cfg(not(target_os = "linux"))]
pub fn io_counters() -> Option<Vec<(String, u64, u64)>> {
    None
}
