use crate::state::Connections;

const ESTABLISHED: &str = "01";
const TIME_WAIT: &str = "06";
const LISTEN: &str = "0A";

/// TCP connection counts and listening ports from /proc/net/tcp[6].
/// Linux only; None elsewhere hides the panel (same pattern as disk I/O).
#[cfg(target_os = "linux")]
pub fn read() -> Option<Connections> {
    let mut any = false;
    let mut conns = Connections {
        established: 0,
        time_wait: 0,
        listening: Vec::new(),
    };
    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        any = true;
        parse_proc_net_tcp(&content, &mut conns);
    }
    conns.listening.sort_unstable();
    conns.listening.dedup();
    conns.listening.truncate(50);
    any.then_some(conns)
}

#[cfg(not(target_os = "linux"))]
pub fn read() -> Option<Connections> {
    None
}

/// Parse one /proc/net/tcp[6] file's content into the running totals.
/// Line format: "sl local_address rem_address st ..." with hex ip:port.
#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn parse_proc_net_tcp(content: &str, conns: &mut Connections) {
    for line in content.lines().skip(1) {
        let mut f = line.split_whitespace();
        let (Some(local), Some(state)) = (f.nth(1), f.nth(1)) else {
            continue;
        };
        match state {
            ESTABLISHED => conns.established += 1,
            TIME_WAIT => conns.time_wait += 1,
            LISTEN => {
                if let Some(port) = local
                    .rsplit(':')
                    .next()
                    .and_then(|p| u16::from_str_radix(p, 16).ok())
                {
                    conns.listening.push(port);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = "\
  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000:1F8D 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12345 1 0000000000000000 100 0 0 10 0
   1: 0100007F:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 12346 1 0000000000000000 100 0 0 10 0
   2: 0100007F:A4F2 0100007F:1F8D 01 00000000:00000000 00:00000000 00000000  1000        0 12347 1 0000000000000000 20 4 30 10 -1
   3: 0100007F:A4F4 0100007F:1F8D 06 00000000:00000000 03:00001234 00000000     0        0 0 3 0000000000000000
";

    #[test]
    fn parses_states_and_listen_ports() {
        let mut c = Connections {
            established: 0,
            time_wait: 0,
            listening: Vec::new(),
        };
        parse_proc_net_tcp(FIXTURE, &mut c);
        assert_eq!(c.established, 1);
        assert_eq!(c.time_wait, 1);
        // 0x1F8D = 8077, 0x16 = 22
        assert_eq!(c.listening, vec![8077, 22]);
    }

    #[test]
    fn tolerates_garbage_lines() {
        let mut c = Connections {
            established: 0,
            time_wait: 0,
            listening: Vec::new(),
        };
        parse_proc_net_tcp("header\nnot a real line\n\n", &mut c);
        assert_eq!(c.established, 0);
        assert_eq!(c.listening.len(), 0);
    }
}
