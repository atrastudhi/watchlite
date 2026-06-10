use crate::state::{Cpu, Host, Memory};
use sysinfo::System;

pub fn host(sys: &System) -> Host {
    Host {
        hostname: System::host_name().unwrap_or_else(|| "unknown".into()),
        os: System::long_os_version()
            .unwrap_or_else(|| System::name().unwrap_or_else(|| "unknown".into())),
        kernel: System::kernel_version().unwrap_or_else(|| "unknown".into()),
        arch: System::cpu_arch(),
        uptime_secs: System::uptime(),
        cpu_count: sys.cpus().len(),
    }
}

pub fn cpu(sys: &System) -> Cpu {
    let load = System::load_average();
    Cpu {
        total_pct: sys.global_cpu_usage(),
        per_core_pct: sys.cpus().iter().map(|c| c.cpu_usage()).collect(),
        load_avg: [load.one, load.five, load.fifteen],
    }
}

pub fn memory(sys: &System) -> Memory {
    Memory {
        total: sys.total_memory(),
        used: sys.used_memory(),
        available: sys.available_memory(),
        swap_total: sys.total_swap(),
        swap_used: sys.used_swap(),
    }
}
