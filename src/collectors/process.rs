use crate::state::{Proc, Processes};
use sysinfo::{ProcessStatus, System};

/// Top-N processes by CPU and by memory, plus state counts. CPU% is percent
/// of a single core (htop convention) — can exceed 100 for multi-threaded
/// processes.
pub fn top(sys: &System, n: usize) -> Processes {
    let mut running = 0;
    let mut sleeping = 0;
    let mut zombie = 0;
    let mut procs: Vec<Proc> = sys
        .processes()
        .values()
        .map(|p| {
            let state = match p.status() {
                ProcessStatus::Run => 'R',
                ProcessStatus::Sleep => 'S',
                ProcessStatus::Idle => 'I',
                ProcessStatus::Zombie => 'Z',
                ProcessStatus::Stop => 'T',
                _ => '?',
            };
            match state {
                'R' => running += 1,
                'S' | 'I' => sleeping += 1,
                'Z' => zombie += 1,
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
    let top_cpu: Vec<Proc> = procs.iter().take(n).cloned().collect();

    procs.sort_unstable_by_key(|p| std::cmp::Reverse(p.mem_bytes));
    let top_mem: Vec<Proc> = procs.into_iter().take(n).collect();

    Processes {
        total,
        running,
        sleeping,
        zombie,
        top_cpu,
        top_mem,
    }
}
