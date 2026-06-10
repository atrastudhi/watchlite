# atrasmon

An ultra-lightweight, single-binary alternative to [Glances](https://nicolargo.github.io/glances/) for server monitoring.

- **~500 KB static binary**, ~10 MB RSS, ~0.1% CPU — no runtime, no dependencies, nothing to install
- **Embedded web dashboard** (vanilla HTML/CSS/JS, dark htop-style theme) served by the binary itself
- **Metrics**: CPU (total + per-core), memory/swap, load average, disk usage + I/O rates, network throughput, top processes, Docker containers
- Docker stats via the unix socket with a hand-rolled client — no daemon polling cost, gracefully hidden when Docker is absent

## Usage

```sh
atrasmon                                  # serves http://127.0.0.1:8077
atrasmon --bind 0.0.0.0:8077 --auth admin:secret   # remote access with basic auth
```

| Flag | Default | Description |
|---|---|---|
| `--bind <ADDR>` | `127.0.0.1:8077` | Listen address (`0.0.0.0:...` for remote access) |
| `--interval <SECS>` | `2` | Sampling interval (0.5–3600) |
| `--top <N>` | `10` | Top processes to report (1–100) |
| `--no-docker` | | Disable the Docker collector |
| `--auth <USER:PASS>` | | Require HTTP Basic auth |

Env-var equivalents: `ATRASMON_BIND`, `ATRASMON_INTERVAL`, `ATRASMON_TOP`, `ATRASMON_AUTH` (flags win).

## API

`GET /api/stats` returns the latest snapshot as JSON (one sample per interval; rates are bytes/sec computed server-side from counter deltas). `disk_io` is `null` on non-Linux hosts; `docker` is `null` when the Docker socket is unavailable.

## Build

```sh
cargo build --release          # native
```

Fully static Linux binary (deploy by copying one file):

```sh
docker run --rm -v "$PWD":/app -w /app rust:alpine \
  sh -c "apk add musl-dev && cargo build --release --target x86_64-unknown-linux-musl"
```

## Run as a service (systemd)

```ini
[Unit]
Description=atrasmon
After=network.target

[Service]
ExecStart=/usr/local/bin/atrasmon --bind 0.0.0.0:8077 --auth admin:CHANGE_ME
Restart=always
DynamicUser=yes
# Docker panel needs socket access; remove if unused:
SupplementaryGroups=docker

[Install]
WantedBy=multi-user.target
```
