# Contributing to watchlite

Thanks for your interest! A few ground rules keep this project what it is.

## The prime directive: stay light

watchlite exists because the alternatives are heavy. Every change is judged against that:

- **No new runtime dependencies** unless they're tiny and irreplaceable. No tokio, no clap, no chart libraries. If it can be done in 50 lines of std, do that.
- The release binary should stay **under 1 MB**, RSS under ~15 MB.
- The HTTP request path must stay zero-work (responses are pre-rendered once per sampling tick).
- Features that need heavy platform libraries (NVML, libatasmart, …) are out of scope; an opt-in shell-out to an existing tool may be acceptable.

If your idea conflicts with these, open an issue to discuss before writing code.

## Development

```sh
cargo build && cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
./target/debug/watchlite   # http://127.0.0.1:8077
```

The dashboard is vanilla HTML/CSS/JS in `src/static/`, embedded via `include_str!` — no build step; just edit and `cargo build`.

Linux-only collectors (`/proc` parsers) keep their parsing logic in pure functions that take string input, so they're unit-testable on any OS — follow that pattern (see `collectors/connections.rs`).

## Commit messages

Releases are automated with [release-please](https://github.com/googleapis/release-please): version bumps and the changelog are derived from [Conventional Commits](https://www.conventionalcommits.org/). Prefix your commits accordingly:

- `fix: ...` → patch release, listed under Bug Fixes
- `feat: ...` → minor release, listed under Features
- `feat!: ...` or a `BREAKING CHANGE:` footer → major release
- `docs:`, `ci:`, `chore:`, `refactor:`, `test:` → no release, not in the changelog

## Pull requests

- One logical change per PR.
- `cargo fmt`, `clippy -D warnings`, and `cargo test` must pass (CI enforces this).
- New parsing logic needs a unit test with a fixture.
- Note the binary-size delta in the PR description if it changes meaningfully (`ls -lh target/release/watchlite`).
