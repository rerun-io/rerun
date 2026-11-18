# `relint`

Rerun's custom linter for Rust source code, for conventions that `clippy` cannot check.

`relint` parses Rust into a lossless syntax tree with [`ra_ap_syntax`](https://crates.io/crates/ra_ap_syntax) (the parser of rust-analyzer).
However, there is no name resolution, no type information, and no macro expansion.

`relint` is fast to compile and fast to run.

`relint` itself is vibe-coded.

## Running

From `rerun/`:

```sh
pixi run relint                       # all .rs and .md files git knows about
pixi run relint crates/utils/re_log   # one directory
pixi run relint --fix                 # apply the automatic fixes (e.g. missing blank lines)
pixi run relint-self-check            # fmt, clippy, and tests of the linter itself
```

From `dataplatform/`: `pixi run relint`.

Files that `.gitattributes` marks as `linguist-generated` are skipped.
In markdown files and doc comments, `relint` lints the Rust code blocks, with the rules that make sense for docs snippets.
Rules that do not apply to a project are disabled in a `relint.toml`, which applies to all files under its directory.
`relint --list-rules` lists all rules.

## Allowing a rule

Put `// relint: allow(rule_name): reason` at the end of the line the diagnostic points to, or on its own line above it.
A marker on its own line applies to the next line that is not a `//` comment, so put it below any attributes such as `#[allow(…)]`.
The reason is required:

```rust
// relint: allow(debug_formatted_error): `puffin::Error` only implements `Debug`
let reader = Reader::new(stream).map_err(|err| anyhow!("invalid stream: {err:?}"))?;
```

A marker that does not suppress anything is itself an error (`unused_allow`), so markers cannot outlive the code they were written for.

## Adding a rule

1. Add `src/rules/your_rule.rs`, modeled on an existing rule, with tests for the corner cases.
2. Register it in `RULES` in `src/rules/mod.rs` (sorted by name).
3. Run `pixi run relint` in both `rerun/` and `dataplatform/`, and fix or allow what it finds.

## Why a separate cargo workspace?

`ra_ap_*` crates publish a breaking release every week.
Keeping them out of the main `Cargo.lock` keeps those bumps out of Dependabot, `cargo deny`, and the main build.
