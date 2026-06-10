mod collectors;
mod config;
mod http;
mod sampler;
mod state;

use std::sync::{Arc, Mutex};

fn main() {
    let config = config::Config::from_args();

    // Placeholder until the first sample lands (~interval after startup).
    let state: state::SharedState = Arc::new(Mutex::new("{\"warming_up\":true}".to_string()));

    let sampler_state = state.clone();
    let sampler_config = config.clone();
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || sampler::run(sampler_state, sampler_config))
        .expect("failed to spawn sampler thread");

    http::serve(&config, state);
}
