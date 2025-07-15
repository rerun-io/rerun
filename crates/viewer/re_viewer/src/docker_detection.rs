/// Detect if the application is running inside a Docker container
#[cfg(target_os = "linux")]
pub fn is_docker() -> bool {
    /// Check for the presence of /.dockerenv file (most reliable method)
    fn is_dockerenv_present() -> bool {
        std::path::Path::new("/.dockerenv").exists()
    }

    /// Check if 'docker' appears in cgroup information
    fn is_docker_in_cgroup() -> bool {
        // Try multiple cgroup paths
        let cgroup_paths = ["/proc/1/cgroup", "/proc/self/cgroup"];

        for path in &cgroup_paths {
            if let Ok(contents) = std::fs::read_to_string(path) {
                if contents.contains("docker") {
                    return true;
                }
            }
        }
        false
    }

    is_dockerenv_present() || is_docker_in_cgroup()
}

/// Detect if the application is running inside a Docker container
#[cfg(not(target_os = "linux"))]
pub fn is_docker() -> bool {
    false
}
