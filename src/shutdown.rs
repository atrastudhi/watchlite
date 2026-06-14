//! SIGTERM/SIGINT handling so the sampler can flush chart history before the
//! process exits (otherwise `systemctl stop` / Ctrl-C drops up to a minute of
//! it, since persistence is periodic). No dependency: we declare libc's
//! `signal` directly — it's already linked, and the handler only flips an
//! atomic, which is async-signal-safe.

#[cfg(unix)]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    static REQUESTED: AtomicBool = AtomicBool::new(false);

    const SIGINT: i32 = 2;
    const SIGTERM: i32 = 15;

    type Handler = extern "C" fn(i32);

    extern "C" {
        fn signal(signum: i32, handler: Handler) -> usize;
    }

    extern "C" fn on_signal(_sig: i32) {
        REQUESTED.store(true, Ordering::SeqCst);
    }

    pub fn install() {
        // SAFETY: registering a handler that only does an atomic store.
        unsafe {
            signal(SIGINT, on_signal);
            signal(SIGTERM, on_signal);
        }
    }

    pub fn requested() -> bool {
        REQUESTED.load(Ordering::SeqCst)
    }
}

#[cfg(not(unix))]
mod imp {
    pub fn install() {}
    pub fn requested() -> bool {
        false
    }
}

pub use imp::{install, requested};
