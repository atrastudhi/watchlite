//! --check-update: explicit one-shot version check against GitHub releases.
//! Only runs when the user asks — watchlite never phones home on its own.

/// Derived from Cargo.toml's repository field so forks stay correct.
const REPO_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// Exit codes: 0 = up to date, 1 = check failed, 2 = update available.
pub fn check() -> i32 {
    let current = env!("CARGO_PKG_VERSION");
    let slug = REPO_URL.trim_start_matches("https://github.com/");
    let api = format!("https://api.github.com/repos/{slug}/releases/latest");
    let out = std::process::Command::new("curl")
        .args(["-fsSL", "-m", "10", "-H", "User-Agent: watchlite", &api])
        .output();
    let body = match out {
        Ok(o) if o.status.success() => o.stdout,
        _ => {
            eprintln!(
                "error: could not reach the releases API (is curl installed and the network up?)"
            );
            return 1;
        }
    };
    let latest = serde_json::from_slice::<serde_json::Value>(&body)
        .ok()
        .and_then(|v| v["tag_name"].as_str().map(String::from));
    let Some(latest) = latest else {
        eprintln!("error: unexpected response from the releases API");
        return 1;
    };

    if is_newer(&latest, current) {
        println!("watchlite {current} -> {latest} available");
        println!("  binary: re-run the install one-liner from the README");
        println!("  cargo:  cargo install watchlite");
        println!("  docker: docker pull ghcr.io/{slug}:latest");
        println!("  {REPO_URL}/releases/latest");
        2
    } else {
        println!("watchlite {current} is up to date");
        0
    }
}

/// Strict semver triple comparison; anything unparseable is "not newer".
fn is_newer(latest: &str, current: &str) -> bool {
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

fn parse(v: &str) -> Option<(u64, u64, u64)> {
    let mut it = v.trim().trim_start_matches('v').splitn(3, '.');
    Some((
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
        it.next()?.parse().ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn compares_versions() {
        assert!(is_newer("v0.9.1", "0.9.0"));
        assert!(is_newer("v1.0.0", "0.99.99"));
        assert!(!is_newer("v0.9.0", "0.9.0"));
        assert!(!is_newer("v0.8.9", "0.9.0"));
        assert!(!is_newer("not-a-version", "0.9.0"));
        assert!(is_newer("0.10.0", "0.9.0"), "numeric, not lexicographic");
    }
}
