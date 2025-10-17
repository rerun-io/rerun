# re_lint

Custom linter for the Rerun codebase.

This is a Rust port of `scripts/lint.py` that provides the same linting functionality with better performance and integration.

## Features

- Line-by-line linting for common style issues
- Rust-specific linting (vertical spacing, pyclass checks, workspace lints)
- Markdown-specific linting (header casing, capitalization rules)
- Automatic fixing of some issues with `--fix` flag
- Uses the `ignore` crate to respect `.gitignore` files

## Usage

```bash
# Lint all files in the repository
cargo run --bin re_lint

# Lint specific files
cargo run --bin re_lint path/to/file1.rs path/to/file2.md

# Automatically fix issues
cargo run --bin re_lint --fix

# Run extra checks
cargo run --bin re_lint --extra
```

## Ignored Lines

Add `NOLINT` to any line to make the linter ignore that line.
Add a pair of `NOLINT_START` and `NOLINT_END` to ignore multiple lines.

## Implementation

The linter is organized into several modules:

- `main.rs` - Entry point and file walking logic using the `ignore` crate
- `lint_rules.rs` - Line-by-line linting rules
- `rust_lints.rs` - Rust-specific linting (vertical spacing, pyclass, etc.)
- `markdown.rs` - Markdown-specific linting
