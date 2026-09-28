use std::{
    fmt::Display,
    fs::OpenOptions,
    io::Write,
    sync::OnceLock,
    time::{SystemTime, UNIX_EPOCH},
};

/// Diagnostic logging is opt-in because this function is called from input
/// and network paths. Opening and appending a file on those paths makes the
/// render loop compete with disk I/O and can produce visible hitching.
pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("GRAVAK_DEBUG_LOG")
            .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false)
    })
}

pub fn write(message: impl Display) {
    if !enabled() {
        return;
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let line = format!("[{timestamp}] [DRAGDROP] {message}");
    eprintln!("{line}");
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("dragdrop-debug.log")
    {
        let _ = writeln!(file, "{line}");
    }
}
