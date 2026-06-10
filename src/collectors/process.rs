use crate::state::{Proc, Processes};
use sysinfo::{ProcessStatus, System};

/// Full process list sorted by CPU descending, plus state counts. CPU% is
/// percent of a single core (htop convention) — can exceed 100 for
/// multi-threaded processes. `cap` of 0 means unlimited.
pub fn list(sys: &System, cap: usize) -> Processes {
    let mut running = 0;
    let mut sleeping = 0;
    let mut zombie = 0;
    let mut procs: Vec<Proc> = sys
        .processes()
        .values()
        .map(|p| {
            let state = match p.status() {
                ProcessStatus::Run => "running",
                ProcessStatus::Sleep => "sleeping",
                ProcessStatus::Idle => "idle",
                ProcessStatus::Zombie => "zombie",
                ProcessStatus::Stop => "stopped",
                _ => "unknown",
            };
            match state {
                "running" => running += 1,
                "sleeping" | "idle" => sleeping += 1,
                "zombie" => zombie += 1,
                _ => {}
            }
            Proc {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                state,
                cpu_pct: p.cpu_usage(),
                mem_bytes: p.memory(),
            }
        })
        .collect();
    let total = procs.len();

    procs.sort_unstable_by(|a, b| b.cpu_pct.total_cmp(&a.cpu_pct));
    if cap > 0 {
        procs.truncate(cap);
    }

    Processes {
        total,
        running,
        sleeping,
        zombie,
        list: procs,
    }
}
