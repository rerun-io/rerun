"""Reusable Bazel macros for simple Rerun Rust crates."""

load("@rules_rust//rust:defs.bzl", "rust_doc", "rust_doc_test", "rust_library", "rust_test")
load("@crates//:defs.bzl", "all_crate_deps")

def re_simple_crate(
        name,
        srcs = None,
        deps = None,
        test_size = "small",
        compile_data = None,
        **kwargs):
    """Define a simple Rust crate with library and test targets.

    Args:
        name: Name of the crate (used for the library target)
        srcs: Source files (defaults to glob(["src/**/*.rs"]))
        deps: Additional dependencies beyond all_crate_deps
        test_size: Size of the test target (default: "small")
        compile_data: Additional compile-time data files beyond Cargo.toml
        **kwargs: Additional arguments passed to rust_library
    """
    if srcs == None:
        srcs = native.glob(["src/**/*.rs"])

    all_deps = all_crate_deps(normal = True)
    if deps:
        all_deps = all_deps + deps

    all_proc_macro_deps = all_crate_deps(proc_macro = True)
    all_dev_deps = all_crate_deps(normal_dev = True)
    all_proc_macro_dev_deps = all_crate_deps(proc_macro_dev = True)

    # Make Cargo.toml available for proc-macros that need it (like document-features)
    all_compile_data = ["Cargo.toml"]
    if compile_data:
        all_compile_data = all_compile_data + compile_data

    # Set CARGO_MANIFEST_DIR so proc-macros can find Cargo.toml
    # Use $${pwd} to get absolute path in the sandbox
    rustc_env = {
        "CARGO_MANIFEST_DIR": "$${pwd}/" + native.package_name(),
    }

    # Library target
    rust_library(
        name = name,
        srcs = srcs,
        deps = all_deps,
        proc_macro_deps = all_proc_macro_deps,
        compile_data = all_compile_data,
        rustc_env = rustc_env,
        **kwargs
    )

    # Test target
    rust_test(
        name = "test",
        crate = ":" + name,
        size = test_size,
        deps = all_deps + all_dev_deps,
        proc_macro_deps = all_proc_macro_deps + all_proc_macro_dev_deps,
    )

    # Doc test target
    rust_doc_test(
        name = "doctest",
        crate = ":" + name,
        size = "small",
        deps = all_deps + all_dev_deps,
        proc_macro_deps = all_proc_macro_deps + all_proc_macro_dev_deps,
    )

    # Documentation target
    rust_doc(
        name = "doc",
        crate = ":" + name,
    )
