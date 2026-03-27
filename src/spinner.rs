use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

pub struct Spinner {
    bar: ProgressBar,
}

impl Spinner {
    pub fn start(label: &str) -> Self {
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "])
                .template("{spinner} {msg}")
                .expect("valid spinner template"),
        );
        bar.set_message(format!("memento {label}"));
        bar.enable_steady_tick(Duration::from_millis(80));

        Spinner { bar }
    }

    pub fn stop(self) {
        self.bar.finish_and_clear();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.bar.finish_and_clear();
    }
}
