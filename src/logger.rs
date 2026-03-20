use std::fmt;
use std::io::{self, IsTerminal, Write};
use std::sync::OnceLock;

static LOGGER: OnceLock<Logger> = OnceLock::new();

pub fn init() {
    let _ = LOGGER.get_or_init(Logger::detect);
}

pub fn write_line(args: fmt::Arguments<'_>) {
    LOGGER.get_or_init(Logger::detect).write_line(args);
}

struct Logger {
    sink: Sink,
}

impl Logger {
    fn detect() -> Self {
        let has_terminal = io::stdout().is_terminal() || io::stderr().is_terminal();
        let sink = if has_terminal {
            Sink::Stderr
        } else {
            Sink::Silent
        };

        Self { sink }
    }

    fn write_line(&self, args: fmt::Arguments<'_>) {
        match self.sink {
            Sink::Stderr => {
                let mut stderr = io::stderr().lock();
                let _ = stderr.write_fmt(args);
                let _ = stderr.write_all(b"\n");
            }
            Sink::Silent => {}
        }
    }
}

#[derive(Clone, Copy)]
enum Sink {
    Stderr,
    Silent,
}

#[macro_export]
macro_rules! logln {
    ($($arg:tt)*) => {
        $crate::logger::write_line(format_args!($($arg)*))
    };
}
