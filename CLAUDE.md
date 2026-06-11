# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

watchlite — an ultra-lightweight single-binary server monitor with an embedded web dashboard. The defining constraint is footprint: **the release binary must stay under 1 MiB** (CI-enforced), RSS ~11 MB. Every design decision flows from this. Do not add dependencies without strong justification — no tokio, no clap, no bollard, no TLS stacks (webhooks shell out to `curl` instead). Current deps: `tiny_http`, `sysinfo`, `serde`, `serde_json` — that's the whole tree.

Do not mention Glances in the README (explicit project decision; positioning is standalone).

## Commands

```sh
cargo build --release            # native binary -> target/release/watchlite
cargo test                       # all unit tests
cargo test payload               # single test by name substring
cargo fmt --all                  # required; CI runs --check
cargo clippy --all-targets -- -D warnings   # CI gate, must be clean
./target/release/watchlite       # serves http://127.0.0.1:8077
./target/release/watchlite --once | jq .   # one-shot snapshot, no server
```

Static Linux build (the deployment artifact) — must go through Docker on this Mac, and **`--platform linux/amd64` is mandatory** (without it Docker may reuse a cached arm64 image; and building in-container without `--target` overwrites the native binary in the shared `target/release/`):

```sh
docker run --rm --platform linux/amd64 -v "$PWD":/app -w /app rust:alpine \
  sh -c "apk add -q musl-dev && cargo build --release --target x86_64-unknown-linux-musl"
```

MSRV is 1.95 (floor set by sysinfo, CI-enforced). sysinfo's API churns across 0.x versions — treat its upgrades as migrations, not routine bumps.

## NEVER push without explicit user approval

Every push to master containing `feat:`/`fix:` commits immediately cuts a public release to GitHub, crates.io, and GHCR. To keep the version history meaningful: **commit locally as much as needed, but only `git push` when the user explicitly says to.** Batch related work — one push containing several commits produces a single release (the highest bump wins), which is the intended way to avoid version churn.

## Releases are fully automated — never do them manually

One workflow (`.github/workflows/ci.yml`) does everything on pushes to master: check/msrv/smoke gates → version bump → CHANGELOG section → annotated tag → 4-target binaries → GitHub release → crates.io (OIDC trusted publishing) → multi-arch GHCR image.

- **Never** edit the version in Cargo.toml, write CHANGELOG entries, or create tags by hand — the pipeline derives all three from conventional commit subjects (`feat:` → minor, `fix:`/`perf:` → patch, `feat!:`/`BREAKING CHANGE` → major; `docs:`/`ci:`/`chore:` release nothing).
- Commit subjects become changelog lines verbatim — write them for end users.
- The bot pushes a `chore: release vX.Y.Z` commit to master after each release; **`git pull --rebase` before pushing** or your push will be rejected.
- The release commit/tag are pushed with the Actions token, which never triggers workflows — that's why there's no release loop.

## Architecture: render-once-per-tick

The whole backend is built around one invariant: **HTTP request handling does zero work**. A single sampler thread (`src/sampler.rs`) wakes every `--interval` (default 2s), collects everything, and pre-renders both the JSON snapshot and the Prometheus text into `Shared` (`src/state.rs`, three mutexed strings/buffers). Handlers (`src/http/mod.rs`, 4 threads on one `tiny_http` server) just clone strings. Only `/api/history` serializes on demand (page loads only).

Rate metrics (network/disk/container CPU) are deltas of cumulative counters: the sampler keeps a `Prev` struct of last-tick counters, divides by **measured** elapsed time (not the configured interval), uses `saturating_sub` against counter resets, and rebuilds the maps from current keys each tick so device/container churn can't grow them.

### Collectors (`src/collectors/`)

- Cross-platform metrics come from `sysinfo` with narrow `RefreshKind`s (only what's displayed).
- Linux-only metrics (`disk.rs` I/O via `/proc/diskstats`, `connections.rs` via `/proc/net/tcp[6]`) are cfg-gated and return `None` elsewhere — the UI then shows a "not supported" note. **Pattern: keep `/proc` parsing in pure functions taking `&str`** so they unit-test on any OS with fixture strings.
- `docker.rs` is a hand-rolled HTTP client over `UnixStream` (incl. chunked-transfer decoding). It probes Docker then rootful/rootless Podman sockets; **API paths must stay unversioned** (`/containers/json`, not `/v1.41/...`) — modern engines reject old pinned versions, old engines don't know new ones. Container CPU% is computed from our own prev-tick counters using `one-shot=true` stats (avoids the daemon's 1s blocking delta).
- Alerts (`src/alerts.rs`): 3-tick hysteresis both directions; `emit()` fires only on firing/resolved transitions; Discord webhook URLs are auto-detected by substring and get Discord's payload shape.

### Frontend (`src/static/`)

Vanilla HTML/CSS/JS embedded into the binary via `include_str!` (`src/http/assets.rs`) — **no build step, but editing static files requires a cargo rebuild** to re-embed. The UI polls `/api/stats`, seeds charts from `/api/history` on load, and does all table sorting/filtering client-side (the API ships the full process list). Canvas charts, no libraries. Keep the dense one-screen layout; lists scroll within panels.

### Platform-absence convention

Every optional metric is `Option` in the JSON (`disk_io`, `connections`, `sensors`, `docker` are `null` when unavailable) and the UI hides the panel or shows "not supported" — collectors must degrade silently, never crash or log-spam (log once on state change).

## Verification beyond unit tests

CI's smoke job runs the real musl binary and curls every endpoint (including auth 401/200) and enforces the size budget — mirror that locally for risky changes: run the binary, check `/api/stats` JSON, the dashboard in a browser, and `ls -lh` the binary. The README gif (`assets/dashboard.gif`) is regenerated from a deterministic stubbed-API harness (seeded PRNG, `?t=N` step param, headless Chrome frames + ffmpeg palettegen) — rebuild it after visible UI changes, never from real data (leaks hostname/processes).

## Shell gotcha

`--alert cpu>90` must be quoted in shells (`'cpu>90'`) or `>` redirects — but **not** in systemd `ExecStart`, which has no shell.
