use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

#[derive(Clone)]
pub struct Config {
    pub bind: SocketAddr,
    pub interval: Duration,
    pub top_n: usize,
    pub docker: bool,
    /// Precomputed `Basic <base64(user:pass)>` value, if auth is enabled.
    pub auth: Option<String>,
    /// How much sample history to keep in RAM.
    pub history: Duration,
    /// Where chart history is persisted across restarts; None disables.
    pub history_file: Option<PathBuf>,
    pub alerts: Vec<AlertSpec>,
    pub webhook: Option<String>,
}

#[derive(Clone)]
pub struct AlertSpec {
    /// "cpu", "mem", or "disk" — all percentages.
    pub metric: String,
    pub threshold: f64,
}

const USAGE: &str = "\
watchlite - ultra-lightweight server monitor

USAGE:
    watchlite [OPTIONS]

OPTIONS:
    --bind <ADDR>       Address to listen on (default: 127.0.0.1:8077)
                        Use 0.0.0.0:8077 to allow remote access.
    --interval <SECS>   Sampling interval in seconds (default: 2)
    --top <N>           Cap the process list sent to the UI (0 = all, default)
    --no-docker         Disable the Docker collector
    --auth <USER:PASS>  Require HTTP Basic auth
    --history <SECS>    Sample history kept in RAM (default: 3600)
    --history-file <P>  Persist chart history to this file, saved once a
                        minute (pass 'none' to disable). Default:
                        $STATE_DIRECTORY (systemd) or ~/.local/state/watchlite/
    --alert <SPEC>      Alert rule, repeatable. SPEC is metric>percent,
                        metric one of cpu, mem, disk. Example: --alert cpu>90
    --webhook <URL>     POST alert events as JSON (uses curl; needed for https)
    --version           Print version
    --help              Show this help

ENVIRONMENT (flags take precedence):
    WATCHLITE_BIND, WATCHLITE_INTERVAL, WATCHLITE_TOP, WATCHLITE_AUTH,
    WATCHLITE_HISTORY, WATCHLITE_HISTORY_FILE, WATCHLITE_WEBHOOK
";

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}\n\n{USAGE}");
    exit(2);
}

impl Config {
    pub fn from_args() -> Config {
        let mut bind = env::var("WATCHLITE_BIND").unwrap_or_else(|_| "127.0.0.1:8077".into());
        let mut interval = env::var("WATCHLITE_INTERVAL").unwrap_or_else(|_| "2".into());
        let mut top_n = env::var("WATCHLITE_TOP").unwrap_or_else(|_| "0".into());
        let mut auth = env::var("WATCHLITE_AUTH").ok();
        let mut history = env::var("WATCHLITE_HISTORY").unwrap_or_else(|_| "3600".into());
        let mut history_file_arg = env::var("WATCHLITE_HISTORY_FILE").ok();
        let mut webhook = env::var("WATCHLITE_WEBHOOK").ok();
        let mut alert_specs: Vec<String> = Vec::new();
        let mut docker = true;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let mut take = |name: &str| {
                args.next()
                    .unwrap_or_else(|| fail(&format!("{name} requires a value")))
            };
            match arg.as_str() {
                "--bind" => bind = take("--bind"),
                "--interval" => interval = take("--interval"),
                "--top" => top_n = take("--top"),
                "--auth" => auth = Some(take("--auth")),
                "--history" => history = take("--history"),
                "--history-file" => history_file_arg = Some(take("--history-file")),
                "--alert" => alert_specs.push(take("--alert")),
                "--webhook" => webhook = Some(take("--webhook")),
                "--no-docker" => docker = false,
                "--version" | "-V" => {
                    println!("watchlite {}", env!("CARGO_PKG_VERSION"));
                    exit(0);
                }
                "--help" | "-h" => {
                    print!("{USAGE}");
                    exit(0);
                }
                other => fail(&format!("unknown flag: {other}")),
            }
        }

        let bind: SocketAddr = bind
            .parse()
            .unwrap_or_else(|_| fail(&format!("invalid bind address: {bind}")));
        let secs: f64 = interval
            .parse()
            .ok()
            .filter(|s| *s >= 0.5 && *s <= 3600.0)
            .unwrap_or_else(|| fail(&format!("invalid interval (0.5-3600): {interval}")));
        let top_n: usize = top_n
            .parse()
            .ok()
            .filter(|n| *n <= 10000)
            .unwrap_or_else(|| fail(&format!("invalid top count (0-10000, 0=all): {top_n}")));
        let auth = auth.map(|creds| {
            if !creds.contains(':') {
                fail("--auth must be in USER:PASS form");
            }
            format!("Basic {}", base64_encode(creds.as_bytes()))
        });
        let history: u64 = history
            .parse()
            .ok()
            .filter(|h| *h >= 60 && *h <= 86400)
            .unwrap_or_else(|| fail(&format!("invalid history seconds (60-86400): {history}")));
        let alerts = alert_specs
            .iter()
            .map(|spec| {
                let (metric, threshold) = spec.split_once('>').unwrap_or_else(|| {
                    fail(&format!("invalid alert spec (want metric>percent): {spec}"))
                });
                if !["cpu", "mem", "disk"].contains(&metric) {
                    fail(&format!("unknown alert metric (cpu, mem, disk): {metric}"));
                }
                let threshold: f64 = threshold
                    .parse()
                    .ok()
                    .filter(|t| *t > 0.0 && *t < 100.0)
                    .unwrap_or_else(|| fail(&format!("invalid alert threshold: {spec}")));
                AlertSpec {
                    metric: metric.to_string(),
                    threshold,
                }
            })
            .collect();

        Config {
            bind,
            interval: Duration::from_secs_f64(secs),
            top_n,
            docker,
            auth,
            history: Duration::from_secs(history),
            history_file: resolve_history_file(history_file_arg),
            alerts,
            webhook,
        }
    }
}

/// Explicit flag/env wins ("none" disables); otherwise prefer systemd's
/// StateDirectory, then XDG state dir, then ~/.local/state. None when no
/// writable convention applies (e.g. scratch containers) — persistence is
/// simply skipped.
fn resolve_history_file(arg: Option<String>) -> Option<PathBuf> {
    if let Some(p) = arg {
        if p == "none" || p.is_empty() {
            return None;
        }
        return Some(PathBuf::from(p));
    }
    if let Ok(d) = env::var("STATE_DIRECTORY") {
        // may be a colon-separated list; the first entry is ours
        if let Some(first) = d.split(':').next().filter(|s| !s.is_empty()) {
            return Some(PathBuf::from(first).join("history.json"));
        }
    }
    if let Ok(d) = env::var("XDG_STATE_HOME") {
        if !d.is_empty() {
            return Some(PathBuf::from(d).join("watchlite/history.json"));
        }
    }
    if let Ok(h) = env::var("HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h).join(".local/state/watchlite/history.json"));
        }
    }
    None
}

pub fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::base64_encode;

    #[test]
    fn base64_rfc4648_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64_encode(b"admin:secret"), "YWRtaW46c2VjcmV0");
    }
}
