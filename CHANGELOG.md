# Changelog

All notable changes to this project will be documented in this file.
Format follows [Keep a Changelog](https://keepachangelog.com/); versioning follows [SemVer](https://semver.org/).
Entries from 0.1.1 onward are generated automatically by CI from conventional commits.

## [0.12.0](https://github.com/atrastudhi/watchlite/compare/v0.11.0...v0.12.0) (2026-06-14)

### Features

* add a light/dark theme toggle to the dashboard (4418486)
* flush chart history on SIGTERM/SIGINT so graphs survive restarts (c86e687)
* add swap and temperature alerts with per-mount/sensor scoping (0453b0c)


## [0.11.0](https://github.com/atrastudhi/watchlite/compare/v0.10.0...v0.11.0) (2026-06-14)

### Features

* always show the containers panel with the reason it is empty (1ac7c03)


## [0.10.0](https://github.com/atrastudhi/watchlite/compare/v0.9.0...v0.10.0) (2026-06-11)

### Features

* add inline SVG favicon (50c366b)
* add --check-update for explicit version checks (78d7350)

### Bug Fixes

* drop per-state counts from the processes total line (83445e2)


## [0.9.0](https://github.com/atrastudhi/watchlite/compare/v0.8.0...v0.9.0) (2026-06-11)

### Features

* sort docker containers by cpu or mem (52c3ed7)


## [0.8.0](https://github.com/atrastudhi/watchlite/compare/v0.7.1...v0.8.0) (2026-06-11)

### Features

* filter the process table by state with a checklist (aeef0a7)


## [0.7.1](https://github.com/atrastudhi/watchlite/compare/v0.7.0...v0.7.1) (2026-06-11)

### Bug Fixes

* use unversioned container API paths so modern engines work (1ae4906)


## [0.7.0](https://github.com/atrastudhi/watchlite/compare/v0.6.0...v0.7.0) (2026-06-11)

### Features

* send Discord-formatted alerts to Discord webhook URLs (3192e6c)


## [0.6.0](https://github.com/atrastudhi/watchlite/compare/v0.5.0...v0.6.0) (2026-06-11)

### Features

* add --once flag for one-shot JSON snapshots (240c5f8)


## [0.5.0](https://github.com/atrastudhi/watchlite/compare/v0.4.0...v0.5.0) (2026-06-11)

### Features

* support Podman via container engine socket probing (928f70a)


## [0.4.0](https://github.com/atrastudhi/watchlite/compare/v0.3.0...v0.4.0) (2026-06-11)

### Features

* persist chart history across restarts (41f7f57)


## [0.3.0](https://github.com/atrastudhi/watchlite/compare/v0.2.0...v0.3.0) (2026-06-10)

### Features

* publish multi-arch docker images to GHCR on release (4b737e3)


## [0.2.0](https://github.com/atrastudhi/watchlite/compare/v0.1.1...v0.2.0) (2026-06-10)

### Features

* add unauthenticated /healthz endpoint for liveness probes (636aa5f)


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
