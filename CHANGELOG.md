# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/); versioning follows [SemVer](https://semver.org/).
Entries from 0.1.1 onward are generated automatically by CI from conventional commits.

## [0.1.1](https://github.com/atrastudhi/watchlite/compare/v0.1.0...v0.1.1) (2026-06-10)


### Bug Fixes

* stretch charts to full width while history is still filling ([4b80126](https://github.com/atrastudhi/watchlite/commit/4b80126d8ca1bc79648978eaade2ac87b7ba4b93))

## [0.1.0] - 2026-06-10

Initial release.

### Added
- Single static binary serving an embedded web dashboard — dense htop-style
  single-screen layout with gradient area charts, sortable process table,
  zebra tables, live clock, and header alert chip
- Metrics: CPU (model/frequency, total + per-core), memory/swap, load average,
  uptime, disk usage, disk I/O rates (Linux), network throughput, temperatures,
  fan speeds (Linux), TCP connections + listening ports (Linux), full process
  list with states, Docker containers (CPU/memory via the unix socket)
- `/api/stats` JSON endpoint, `/api/history` in-RAM ring buffer (1h default)
  that seeds charts across page reloads, `/metrics` Prometheus text exposition
- Threshold alerts (`--alert cpu>90`) with 3-tick hysteresis, stderr logging,
  and optional webhook delivery via curl
- HTTP Basic auth (`--auth`), localhost bind by default, small handler thread
  pool, no-cache asset serving so binary upgrades show immediately
- Configuration via flags or `WATCHLITE_*` environment variables
- Platform-aware panels: Linux-only collectors show a "not supported" note
  on other platforms instead of hiding
