# Security

watchlite is a **read-only** monitoring tool: it exposes system metrics over HTTP and never accepts commands, writes, or process control.

## Deployment guidance

- By default it binds to `127.0.0.1:8077` — not reachable from the network.
- If you bind to `0.0.0.0`, set `--auth user:pass` (HTTP Basic). Note that Basic auth without TLS is readable on the wire; for internet-facing use, put it behind a TLS reverse proxy (Caddy, nginx) or a VPN/tailnet.
- The metrics themselves are sensitive: hostnames, process names, container names, open ports. Treat the endpoint accordingly.
- The Docker panel only needs *read* access to `/var/run/docker.sock`, but socket access is root-equivalent on most systems — prefer running watchlite under a dedicated user in the `docker` group rather than as root.

## Reporting a vulnerability

Please open a private security advisory on GitHub (Security → Advisories → Report a vulnerability) rather than a public issue. You can expect an initial response within a week.
