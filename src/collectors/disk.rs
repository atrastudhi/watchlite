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
    let content = std::fs::read_to_string("/proc/diskstats").ok()?;
    Some(parse_diskstats(&content, is_whole_device))
}

#[cfg(target_os = "linux")]
fn is_whole_device(name: &str) -> bool {
    // Whole devices (not partitions) appear in /sys/block.
    std::path::Path::new("/sys/block").join(name).exists()
        && !name.starts_with("loop")
        && !name.starts_with("ram")
}

#[cfg(not(target_os = "linux"))]
pub fn io_counters() -> Option<Vec<(String, u64, u64)>> {
    None
}

/// Fields: [2]=device, [5]=sectors read, [9]=sectors written; sector counts
/// are always in 512-byte units regardless of physical sector size.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn parse_diskstats(content: &str, include: impl Fn(&str) -> bool) -> Vec<(String, u64, u64)> {
    const SECTOR: u64 = 512;
    let mut out = Vec::new();
    for line in content.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            continue;
        }
        let name = f[2];
        if !include(name) {
            continue;
        }
        let (Ok(rd), Ok(wr)) = (f[5].parse::<u64>(), f[9].parse::<u64>()) else {
            continue;
        };
        out.push((name.to_string(), rd * SECTOR, wr * SECTOR));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::parse_diskstats;

    const FIXTURE: &str = "\
 259       0 nvme0n1 100 0 2048 50 200 0 4096 80 0 100 130 0 0 0 0 0 0
 259       1 nvme0n1p1 10 0 100 5 20 0 200 8 0 10 13 0 0 0 0 0 0
   7       0 loop0 5 0 50 1 0 0 0 0 0 1 1 0 0 0 0 0 0
short line
";

    #[test]
    fn parses_sectors_to_bytes_with_filter() {
        let got = parse_diskstats(FIXTURE, |n| n == "nvme0n1" || n == "loop0");
        assert_eq!(
            got,
            vec![
                ("nvme0n1".to_string(), 2048 * 512, 4096 * 512),
                ("loop0".to_string(), 50 * 512, 0),
            ]
        );
    }
}
