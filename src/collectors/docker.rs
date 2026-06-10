//! Minimal Docker Engine API client over the unix socket.
//! Deliberately hand-rolled (no bollard/hyper/tokio) to keep the binary tiny.

#![cfg(unix)]

use crate::state::{Container, Docker};
use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

const SOCKET: &str = "/var/run/docker.sock";
const API: &str = "/v1.41";
const MAX_CONTAINERS: usize = 50;

/// Previous-tick CPU counters per container id: (cpu_total_ns, system_cpu_ns).
pub type PrevCpu = HashMap<String, (u64, u64)>;

#[derive(Deserialize)]
struct ContainerSummary {
    #[serde(rename = "Id", default)]
    id: String,
    #[serde(rename = "Names", default)]
    names: Vec<String>,
    #[serde(rename = "Image", default)]
    image: String,
    #[serde(rename = "State", default)]
    state: String,
}

#[derive(Deserialize, Default)]
struct Stats {
    #[serde(default)]
    cpu_stats: CpuStats,
    #[serde(default)]
    memory_stats: MemoryStats,
}

#[derive(Deserialize, Default)]
struct CpuStats {
    #[serde(default)]
    cpu_usage: CpuUsage,
    #[serde(default)]
    system_cpu_usage: u64,
    #[serde(default)]
    online_cpus: u64,
}

#[derive(Deserialize, Default)]
struct CpuUsage {
    #[serde(default)]
    total_usage: u64,
}

#[derive(Deserialize, Default)]
struct MemoryStats {
    #[serde(default)]
    usage: u64,
    #[serde(default)]
    limit: u64,
    #[serde(default)]
    stats: MemStatsInner,
}

#[derive(Deserialize, Default)]
struct MemStatsInner {
    #[serde(default)]
    inactive_file: u64,
    #[serde(default)]
    cache: u64,
}

/// Collect container stats. Returns None on any failure (Docker absent,
/// daemon down, malformed response) — the dashboard hides the panel.
/// `prev_cpu` is replaced with this tick's counters for next-tick deltas.
pub fn collect(prev_cpu: &mut PrevCpu) -> Option<Docker> {
    let body = get(&format!("{API}/containers/json"))?;
    let summaries: Vec<ContainerSummary> = serde_json::from_slice(&body).ok()?;

    let mut next_cpu = PrevCpu::new();
    let mut containers = Vec::new();
    for s in summaries.into_iter().take(MAX_CONTAINERS) {
        let name = s
            .names
            .first()
            .map(|n| n.trim_start_matches('/').to_string())
            .unwrap_or_else(|| s.id.chars().take(12).collect());
        let short_id: String = s.id.chars().take(12).collect();

        let mut c = Container {
            id: short_id,
            name,
            image: s.image,
            state: s.state.clone(),
            cpu_pct: 0.0,
            mem_bytes: 0,
            mem_limit: 0,
        };

        if s.state == "running" {
            if let Some(stats) = get(&format!(
                "{API}/containers/{}/stats?stream=false&one-shot=true",
                s.id
            ))
            .and_then(|b| serde_json::from_slice::<Stats>(&b).ok())
            {
                let cur = (
                    stats.cpu_stats.cpu_usage.total_usage,
                    stats.cpu_stats.system_cpu_usage,
                );
                if let Some(&(prev_total, prev_sys)) = prev_cpu.get(&s.id) {
                    let cpu_delta = cur.0.saturating_sub(prev_total) as f64;
                    let sys_delta = cur.1.saturating_sub(prev_sys) as f64;
                    if sys_delta > 0.0 {
                        let cores = stats.cpu_stats.online_cpus.max(1) as f64;
                        c.cpu_pct = (cpu_delta / sys_delta * cores * 100.0 * 10.0).round() / 10.0;
                    }
                }
                next_cpu.insert(s.id.clone(), cur);

                let m = &stats.memory_stats;
                // cgroup v2 reports inactive_file; v1 reports cache. Subtract
                // whichever is present so page cache doesn't count as "used".
                let reclaimable = if m.stats.inactive_file > 0 {
                    m.stats.inactive_file
                } else {
                    m.stats.cache
                };
                c.mem_bytes = m.usage.saturating_sub(reclaimable);
                c.mem_limit = m.limit;
            }
        }
        containers.push(c);
    }

    *prev_cpu = next_cpu;
    Some(Docker { containers })
}

/// One HTTP/1.1 GET over the unix socket. Connection: close, so we read to
/// EOF and then deal with Content-Length vs chunked framing.
fn get(path: &str) -> Option<Vec<u8>> {
    let mut stream = UnixStream::connect(SOCKET).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: docker\r\nConnection: close\r\n\r\n"
    )
    .ok()?;

    let mut raw = Vec::with_capacity(8192);
    stream.take(8 * 1024 * 1024).read_to_end(&mut raw).ok()?;

    let header_end = raw.windows(4).position(|w| w == b"\r\n\r\n")? + 4;
    let headers = std::str::from_utf8(&raw[..header_end]).ok()?;
    let status = headers.split_whitespace().nth(1)?;
    if status != "200" {
        return None;
    }
    let body = &raw[header_end..];

    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        decode_chunked(body)
    } else {
        Some(body.to_vec())
    }
}

/// Decode HTTP/1.1 chunked transfer encoding: hex-size line, chunk bytes,
/// CRLF, repeated until a zero-size chunk.
fn decode_chunked(mut body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(body.len());
    loop {
        let line_end = body.windows(2).position(|w| w == b"\r\n")?;
        let size_str = std::str::from_utf8(&body[..line_end]).ok()?;
        let size = usize::from_str_radix(size_str.trim().split(';').next()?.trim(), 16).ok()?;
        if size == 0 {
            return Some(out);
        }
        let start = line_end + 2;
        let end = start + size;
        if end > body.len() {
            return None;
        }
        out.extend_from_slice(&body[start..end]);
        body = body.get(end + 2..)?;
    }
}

#[cfg(test)]
mod tests {
    use super::decode_chunked;

    #[test]
    fn decodes_chunked_body() {
        let body = b"4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n";
        assert_eq!(decode_chunked(body).unwrap(), b"Wikipedia");
    }

    #[test]
    fn rejects_truncated_chunk() {
        assert!(decode_chunked(b"ff\r\nshort\r\n").is_none());
    }
}
