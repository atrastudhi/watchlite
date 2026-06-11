use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Latest snapshot (JSON) and Prometheus text are pre-rendered once per
/// sampling tick — HTTP handlers just clone strings. History is a compact
/// ring buffer serialized on demand (page loads only).
pub struct Shared {
    pub json: Mutex<String>,
    pub prom: Mutex<String>,
    pub history: Mutex<VecDeque<HistPoint>>,
}

pub type SharedState = Arc<Shared>;

impl Shared {
    pub fn new() -> SharedState {
        Arc::new(Shared {
            json: Mutex::new("{\"warming_up\":true}".to_string()),
            prom: Mutex::new(String::new()),
            history: Mutex::new(VecDeque::new()),
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct HistPoint {
    pub ts: u64,
    pub cpu: f32,
    pub mem: f32,
    pub rx: u64,
    pub tx: u64,
}

#[derive(Serialize)]
pub struct Snapshot {
    pub ts: u64,
    pub interval_secs: f64,
    pub host: Host,
    pub cpu: Cpu,
    pub memory: Memory,
    pub disks: Vec<Disk>,
    pub disk_io: Option<Vec<DiskIo>>,
    pub net: Vec<Net>,
    pub processes: Processes,
    pub docker: Option<Docker>,
    pub sensors: Option<Sensors>,
    pub connections: Option<Connections>,
    pub alerts: Vec<AlertStatus>,
}

#[derive(Serialize)]
pub struct Connections {
    pub established: u32,
    pub time_wait: u32,
    pub listening: Vec<u16>,
}

#[derive(Serialize, Clone)]
pub struct AlertStatus {
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub since: u64,
}

#[derive(Serialize)]
pub struct Sensors {
    pub temps: Vec<Temp>,
    pub fans: Vec<Fan>,
}

#[derive(Serialize)]
pub struct Temp {
    pub label: String,
    pub temp_c: f32,
    /// Critical threshold if the sensor reports one.
    pub critical_c: Option<f32>,
}

#[derive(Serialize)]
pub struct Fan {
    pub label: String,
    pub rpm: u64,
}

#[derive(Serialize)]
pub struct Host {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub uptime_secs: u64,
    pub cpu_count: usize,
    /// CPU brand string, e.g. "Apple M3 Pro" or "Intel(R) Xeon(R) ...".
    pub cpu_model: String,
    /// Current/nominal frequency in MHz; 0 when the platform doesn't report it.
    pub cpu_freq_mhz: u64,
}

#[derive(Serialize)]
pub struct Cpu {
    pub total_pct: f32,
    pub per_core_pct: Vec<f32>,
    pub load_avg: [f64; 3],
}

#[derive(Serialize)]
pub struct Memory {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

#[derive(Serialize)]
pub struct Disk {
    pub mount: String,
    pub fs: String,
    pub total: u64,
    pub used: u64,
}

#[derive(Serialize)]
pub struct DiskIo {
    pub device: String,
    pub read_bps: u64,
    pub write_bps: u64,
}

#[derive(Serialize)]
pub struct Net {
    pub iface: String,
    pub rx_bps: u64,
    pub tx_bps: u64,
    pub rx_total: u64,
    pub tx_total: u64,
}

#[derive(Serialize)]
pub struct Processes {
    pub total: usize,
    pub running: usize,
    pub sleeping: usize,
    pub zombie: usize,
    /// All processes sorted by CPU descending (capped by --top if set).
    pub list: Vec<Proc>,
}

#[derive(Serialize, Clone)]
pub struct Proc {
    pub pid: u32,
    pub name: String,
    /// Full state word: running, sleeping, idle, zombie, stopped, unknown.
    pub state: &'static str,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
}

#[derive(Serialize)]
pub struct Docker {
    pub containers: Vec<Container>,
}

#[derive(Serialize)]
pub struct Container {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub cpu_pct: f64,
    pub mem_bytes: u64,
    pub mem_limit: u64,
}
