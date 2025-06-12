use std::io::{Write, stdout};
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

#[derive(Debug)]
pub(crate) struct Task {
    running: AtomicBool,
    text: Box<str>,
}

impl Task {
    pub(crate) fn new(text: &str) -> Self {
        // hide cursor and print message
        let mut stdout = stdout().lock();
        stdout.write_all(text.as_bytes()).unwrap();
        stdout.flush().unwrap();
        Self {
            running: AtomicBool::new(true),
            text: text.into(),
        }
    }

    pub(crate) fn success(&self) {
        if self.running.load(Relaxed) {
            println!(" - Done");
            self.running.store(false, Relaxed);
        }
    }

    pub(crate) fn failure(&self) {
        if self.running.load(Relaxed) {
            println!(" - Failed");
            self.running.store(false, Relaxed);
        }
    }

    pub(crate) fn fix_text(&self) {
        print!("{}", self.text);
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        self.failure();
    }
}
