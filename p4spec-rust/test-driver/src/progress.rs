use std::{
    io::{self, IsTerminal, Write},
    time::Instant,
};

pub struct Progress {
    total: usize,
    start: Instant,
    terminal: bool,
}
impl Progress {
    pub fn new(total: usize) -> Self {
        Self {
            total,
            start: Instant::now(),
            terminal: io::stderr().is_terminal(),
        }
    }
    pub fn show(&self, done: usize, label: &str) {
        let width = 24;
        let filled = done * width / self.total.max(1);
        let bar = format!("{}{}", "=".repeat(filled), " ".repeat(width - filled));
        if self.terminal {
            eprint!("\r\x1b[2K");
        }
        eprint!(
            "[{bar}] {done}/{} {:.1}s {label}",
            self.total,
            self.start.elapsed().as_secs_f64()
        );
        if !self.terminal || done == self.total {
            eprintln!();
        }
        let _ = io::stderr().flush();
    }
}
