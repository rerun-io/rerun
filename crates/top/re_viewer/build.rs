fn main() {
    cfg_aliases::cfg_aliases! {
        // Agent harnesses (Claude Code, Codex, …) are subprocesses, so only the desktop viewer has them.
        agent_harness_backend: { not(target_arch = "wasm32") },
    }

    re_build_tools::export_build_info_vars_for_crate("re_viewer");
}
