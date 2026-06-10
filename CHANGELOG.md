# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/); versioning follows [SemVer](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-06-10

Initial release.

### Added
- Single static binary serving an embedded web dashboard (dark, htop-style)
- Metrics: CPU total/per-core, memory/swap, load average, uptime, disk usage,
  disk I/O rates (Linux), network throughput, temperatures, fan speeds (Linux),
  TCP connections + listening ports (Linux), top processes by CPU/memory with
  state counts, Docker containers (CPU/memory via the unix socket)
- `/api/stats` JSON endpoint, `/api/history` in-RAM ring buffer (1h default),
  `/metrics` Prometheus text exposition
- Threshold alerts (`--alert cpu>90`) with 3-tick hysteresis, stderr logging,
  and optional webhook delivery via curl
- HTTP Basic auth (`--auth`), localhost bind by default
- Configuration via flags or `WATCHLITE_*` environment variables
