//! 全局日志系统

use log::{Level, LevelFilter, Log, Metadata, Record};

/// a simple logger
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let color = match record.level() {
            Level::Error => 31, // Red (致命错误)
            Level::Warn => 33,  // Yellow (标准警告，比 BrightYellow 更柔和)
            Level::Info => 32,  // Green (用户操作成功的暗示)
            Level::Debug => 36, // Cyan (青色，方便在大量文字中一眼扫到)
            Level::Trace => 90, // BrightBlack/Gray (不重要的流水账)
        };
        println!(
            "\u{1B}[{}m[USER][{}] {}\u{1B}[0m",
            color,
            record.level(),
            record.args(),
        );
    }
    fn flush(&self) {}
}

/// 初始化日志系统
pub fn init() {
    static LOGGER: SimpleLogger = SimpleLogger;
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(match option_env!("LOG") {
        Some("ERROR") => LevelFilter::Error,
        Some("WARN") => LevelFilter::Warn,
        Some("INFO") => LevelFilter::Info,
        Some("DEBUG") => LevelFilter::Debug,
        Some("TRACE") => LevelFilter::Trace,
        _ => LevelFilter::Off,
    });
}
