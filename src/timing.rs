use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static TIMING_OVERRIDE: AtomicBool = AtomicBool::new(false);

pub fn enable_timing() {
    TIMING_OVERRIDE.store(true, Ordering::Relaxed);
}

pub fn timing_enabled() -> bool {
    TIMING_OVERRIDE.load(Ordering::Relaxed) || std::env::var_os("MEMENTO_TRACE_TIMING").is_some()
}

pub fn log_timing(label: &str, duration: Duration) {
    if timing_enabled() {
        eprintln!("[memento:timing] {label}={}ms", duration.as_millis());
    }
}

pub fn log_value(label: &str, value: impl std::fmt::Display) {
    if timing_enabled() {
        eprintln!("[memento:timing] {label}={value}");
    }
}
