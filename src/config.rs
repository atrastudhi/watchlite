use std::env;
use std::net::SocketAddr;
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
}

const USAGE: &str = "\
atrasmon - ultra-lightweight server monitor

USAGE:
    atrasmon [OPTIONS]

OPTIONS:
    --bind <ADDR>       Address to listen on (default: 127.0.0.1:8077)
                        Use 0.0.0.0:8077 to allow remote access.
    --interval <SECS>   Sampling interval in seconds (default: 2)
    --top <N>           Number of top processes to report (default: 10)
    --no-docker         Disable the Docker collector
    --auth <USER:PASS>  Require HTTP Basic auth
    --help              Show this help

ENVIRONMENT (flags take precedence):
    ATRASMON_BIND, ATRASMON_INTERVAL, ATRASMON_TOP, ATRASMON_AUTH
";

fn fail(msg: &str) -> ! {
    eprintln!("error: {msg}\n\n{USAGE}");
    exit(2);
}

impl Config {
    pub fn from_args() -> Config {
        let mut bind = env::var("ATRASMON_BIND").unwrap_or_else(|_| "127.0.0.1:8077".into());
        let mut interval = env::var("ATRASMON_INTERVAL").unwrap_or_else(|_| "2".into());
        let mut top_n = env::var("ATRASMON_TOP").unwrap_or_else(|_| "10".into());
        let mut auth = env::var("ATRASMON_AUTH").ok();
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
                "--no-docker" => docker = false,
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
            .filter(|n| *n >= 1 && *n <= 100)
            .unwrap_or_else(|| fail(&format!("invalid top count (1-100): {top_n}")));
        let auth = auth.map(|creds| {
            if !creds.contains(':') {
                fail("--auth must be in USER:PASS form");
            }
            format!("Basic {}", base64_encode(creds.as_bytes()))
        });

        Config {
            bind,
            interval: Duration::from_secs_f64(secs),
            top_n,
            docker,
            auth,
        }
    }
}

pub fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}
