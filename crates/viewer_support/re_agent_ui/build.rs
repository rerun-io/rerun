fn main() {
    cfg_aliases::cfg_aliases! {
        // Agent subprocesses over the Agent Client Protocol need a host with processes.
        agent_harness_backend: { not(target_arch = "wasm32") },
    }
}
