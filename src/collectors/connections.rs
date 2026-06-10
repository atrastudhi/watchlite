use crate::state::Connections;

/// TCP connection counts and listening ports from /proc/net/tcp[6].
/// Linux only; None elsewhere hides the panel (same pattern as disk I/O).
#[cfg(target_os = "linux")]
pub fn read() -> Option<Connections> {
    const ESTABLISHED: &str = "01";
    const TIME_WAIT: &str = "06";
    const LISTEN: &str = "0A";

    let mut established = 0u32;
    let mut time_wait = 0u32;
    let mut listening = std::collections::BTreeSet::new();

    let mut any = false;
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        any = true;
        for line in content.lines().skip(1) {
            let mut f = line.split_whitespace();
            let (Some(local), Some(state)) = (f.nth(1), f.nth(1)) else {
                continue;
            };
            match state {
                ESTABLISHED => established += 1,
                TIME_WAIT => time_wait += 1,
                LISTEN => {
                    if let Some(port) = local
                        .rsplit(':')
                        .next()
                        .and_then(|p| u16::from_str_radix(p, 16).ok())
                    {
                        listening.insert(port);
                    }
                }
                _ => {}
            }
        }
    }

    any.then(|| Connections {
        established,
        time_wait,
        listening: listening.into_iter().take(50).collect(),
    })
}

#[cfg(not(target_os = "linux"))]
pub fn read() -> Option<Connections> {
    None
}
