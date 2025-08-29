use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);

pub fn set(enabled: bool) {
    VERBOSE.store(enabled, Ordering::Relaxed);
}

pub fn enabled() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

// No #[macro_export]; this macro is made visible crate-wide by
// `#[macro_use] mod verbose;` in main.rs.
macro_rules! vprintln {
    ($($arg:tt)*) => {{
        if crate::verbose::enabled() {
            eprintln!($($arg)*);
        }
    }}
}
