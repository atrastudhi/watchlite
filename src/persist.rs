//! Best-effort history persistence: the chart ring buffer is saved to a
//! small JSON file periodically and restored on startup, so dashboards
//! survive restarts/upgrades. Failures never affect monitoring.

use crate::state::HistPoint;
use std::collections::VecDeque;
use std::path::Path;

/// Load saved points, dropping anything older than `min_ts` and keeping at
/// most `cap` newest points. Any read/parse failure yields an empty buffer.
pub fn load(path: &Path, min_ts: u64, cap: usize) -> VecDeque<HistPoint> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return VecDeque::new();
    };
    let Ok(points) = serde_json::from_str::<Vec<HistPoint>>(&content) else {
        return VecDeque::new();
    };
    let mut out: VecDeque<HistPoint> = points.into_iter().filter(|p| p.ts >= min_ts).collect();
    while out.len() > cap {
        out.pop_front();
    }
    out
}

/// Atomic save: write a temp file in the same directory, then rename.
pub fn save(path: &Path, hist: &VecDeque<HistPoint>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string(hist).unwrap_or_else(|_| "[]".into());
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(ts: u64) -> HistPoint {
        HistPoint {
            ts,
            cpu: 1.0,
            mem: 2.0,
            rx: 3,
            tx: 4,
        }
    }

    #[test]
    fn round_trip_with_age_filter_and_cap() {
        let path = std::env::temp_dir().join(format!("watchlite-test-{}.json", std::process::id()));
        let hist: VecDeque<HistPoint> = (100..110).map(point).collect();
        save(&path, &hist).unwrap();

        let all = load(&path, 0, 100);
        assert_eq!(all.len(), 10);
        assert_eq!(all[0].ts, 100);

        let recent = load(&path, 105, 100);
        assert_eq!(recent.len(), 5, "old points dropped");

        let capped = load(&path, 0, 3);
        assert_eq!(capped.len(), 3);
        assert_eq!(capped[0].ts, 107, "keeps newest when capped");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn missing_or_corrupt_file_yields_empty() {
        assert!(load(Path::new("/nonexistent/x.json"), 0, 10).is_empty());
        let path =
            std::env::temp_dir().join(format!("watchlite-corrupt-{}.json", std::process::id()));
        std::fs::write(&path, "not json").unwrap();
        assert!(load(&path, 0, 10).is_empty());
        std::fs::remove_file(&path).unwrap();
    }
}
