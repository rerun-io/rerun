/// Returns true if the current process is running under WSL.
#[cfg(target_os = "linux")]
pub fn is_wsl() -> bool {
    if let Ok(b) = std::fs::read("/proc/sys/kernel/osrelease")
        && let Ok(s) = std::str::from_utf8(&b)
    {
        let a = s.to_ascii_lowercase();
        return a.contains("microsoft") || a.contains("wsl");
    }
    false
}

/// Returns true if the current process is running under WSL.
#[cfg(not(target_os = "linux"))]
pub fn is_wsl() -> bool {
    false
}
