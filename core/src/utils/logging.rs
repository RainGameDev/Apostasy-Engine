use std::sync::OnceLock;

use parking_lot::Mutex;

static LOG_BUFFER: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

pub fn get_log_buffer() -> &'static Mutex<Vec<String>> {
    LOG_BUFFER.get_or_init(|| Mutex::new(Vec::new()))
}

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("[LOG] {}", msg);
        // $crate::get_log_buffer().lock().push(msg);
    }};
}
#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        println!("[WARN] {}", msg);
        // $crate::get_log_buffer().lock().push(msg);
    }
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        println!("[ERROR!] {}", msg);
        // $crate::get_log_buffer().lock().push(msg);
    }
}
