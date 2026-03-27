use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_INTERVAL: Duration = Duration::from_millis(80);

pub struct Spinner {
    active: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Spinner {
    pub fn start(label: &str) -> Self {
        let active = Arc::new(AtomicBool::new(true));
        let thread_active = Arc::clone(&active);
        let message = format!("memento {label}");

        let handle = thread::spawn(move || {
            if !std::io::stderr().is_terminal() {
                return;
            }

            let mut frame_index = 0;
            let stderr = std::io::stderr();

            while thread_active.load(Ordering::Relaxed) {
                let frame = SPINNER_FRAMES[frame_index % SPINNER_FRAMES.len()];
                let mut lock = stderr.lock();
                let _ = write!(lock, "\r{frame} {message}");
                let _ = lock.flush();
                drop(lock);

                frame_index += 1;
                thread::sleep(FRAME_INTERVAL);
            }

            let mut lock = stderr.lock();
            let _ = write!(lock, "\r{}\r", " ".repeat(message.len() + 4));
            let _ = lock.flush();
        });

        Spinner {
            active,
            handle: Some(handle),
        }
    }

    pub fn stop(mut self) {
        self.active.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
