mod alerts;
mod collectors;
mod config;
mod http;
mod prom;
mod sampler;
mod state;

fn main() {
    let config = config::Config::from_args();
    let state = state::Shared::new();

    let sampler_state = state.clone();
    let sampler_config = config.clone();
    std::thread::Builder::new()
        .name("sampler".into())
        .spawn(move || sampler::run(sampler_state, sampler_config))
        .expect("failed to spawn sampler thread");

    http::serve(&config, state);
}
