mod assets;

use crate::config::Config;
use crate::state::SharedState;
use tiny_http::{Header, Request, Response, Server};

pub fn serve(config: &Config, state: SharedState) -> ! {
    let server = Server::http(config.bind).unwrap_or_else(|e| {
        eprintln!("error: failed to bind {}: {e}", config.bind);
        std::process::exit(1);
    });
    eprintln!("watchlite listening on http://{}", config.bind);

    loop {
        let request = match server.recv() {
            Ok(r) => r,
            Err(_) => continue,
        };
        handle(request, config, &state);
    }
}

fn handle(request: Request, config: &Config, state: &SharedState) {
    if let Some(expected) = &config.auth {
        let ok = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Authorization"))
            .is_some_and(|h| constant_time_eq(h.value.as_str(), expected));
        if !ok {
            let resp = Response::from_string("401 Unauthorized")
                .with_status_code(401)
                .with_header(header("WWW-Authenticate", "Basic realm=\"watchlite\""));
            let _ = request.respond(resp);
            return;
        }
    }

    let path = request.url().split('?').next().unwrap_or("/");
    let resp = match path {
        "/" | "/index.html" => asset(assets::INDEX_HTML, assets::CT_HTML),
        "/app.js" => asset(assets::APP_JS, assets::CT_JS),
        "/style.css" => asset(assets::STYLE_CSS, assets::CT_CSS),
        "/api/stats" => {
            let body = lock_or_recover(&state.json).clone();
            Response::from_string(body)
                .with_header(header("Content-Type", assets::CT_JSON))
                .with_header(header("Cache-Control", "no-store"))
        }
        "/api/history" => {
            // Serialized on demand — only hit on page loads, not every tick.
            let body = {
                let hist = lock_or_recover(&state.history);
                serde_json::to_string(&*hist).unwrap_or_else(|_| "[]".into())
            };
            Response::from_string(format!("{{\"points\":{body}}}"))
                .with_header(header("Content-Type", assets::CT_JSON))
                .with_header(header("Cache-Control", "no-store"))
        }
        "/metrics" => {
            let body = lock_or_recover(&state.prom).clone();
            Response::from_string(body)
                .with_header(header(
                    "Content-Type",
                    "text/plain; version=0.0.4; charset=utf-8",
                ))
                .with_header(header("Cache-Control", "no-store"))
        }
        _ => Response::from_string("404 Not Found").with_status_code(404),
    };
    let _ = request.respond(resp);
}

fn asset(body: &'static str, content_type: &str) -> Response<std::io::Cursor<Vec<u8>>> {
    // no-cache so a binary upgrade's embedded UI shows on plain refresh.
    Response::from_string(body)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-cache"))
}

fn lock_or_recover<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn header(field: &str, value: &str) -> Header {
    Header::from_bytes(field.as_bytes(), value.as_bytes()).unwrap()
}

/// Equal-time comparison so auth failures don't leak the match prefix via timing.
fn constant_time_eq(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
