use log::{Level, Metadata, Record, SetLoggerError};
use std::fs::File;
use std::io::{stdout, Stdout, Write};
use std::sync::Mutex;

pub struct MultiLogger {
    level: Level,
    file: Mutex<File>,
    stdout: Mutex<Stdout>,
}

impl MultiLogger {
    pub fn init(file: File, level: Level) -> Result<(), SetLoggerError> {
        let logger = MultiLogger {
            level,
            file: Mutex::new(file),
            stdout: Mutex::new(stdout()),
        };

        log::set_boxed_logger(Box::new(logger))?;
        log::set_max_level(level.to_level_filter());
        Ok(())
    }
}

impl log::Log for MultiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log_msg = format!(
                "[Wine Cask] {} {} {}\n",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.args()
            );

            self.stdout
                .lock()
                .unwrap()
                .write_all(log_msg.as_bytes())
                .unwrap();
            self.file
                .lock()
                .unwrap()
                .write_all(log_msg.as_bytes())
                .unwrap();
        }
    }

    fn flush(&self) {
        self.stdout.lock().unwrap().flush().unwrap();
        self.file.lock().unwrap().flush().unwrap();
    }
}
