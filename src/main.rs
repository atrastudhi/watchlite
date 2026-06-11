mod alerts;
mod collectors;
mod config;
mod http;
mod persist;
mod prom;
mod sampler;
mod state;
mod update;

fn main() {
    let config = config::Config::from_args();
    let state = state::Shared::new();

    let sampler_state = state.clone();
    let sampler_config = config.clone();
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || sampler::run(sampler_state, sampler_config))
        .expect("failed to spawn sampler thread");

    if config.once {
        print_first_snapshot(&state);
        return;
    }

    http::serve(&config, state);
}

/// --once: wait for the sampler's first real snapshot, print it, exit.
/// Logs go to stderr, so stdout is clean JSON for pipelines.
fn print_first_snapshot(state: &state::SharedState) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let json = state.json.lock().unwrap_or_else(|p| p.into_inner()).clone();
        if !json.contains("\"warming_up\"") {
            println!("{json}");
            return;
        }
        if std::time::Instant::now() > deadline {
            eprintln!("error: timed out waiting for the first sample");
            std::process::exit(1);
        }
    }
}
