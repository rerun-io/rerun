//! Finds all the `*.rs` files in `docs/snippets/all`,
//! copies them to `src/snippets` (with slight modifications), and generate a `snippets/mod.rs` for them.
//!
//! The reason we combine all the snippets into a single binary
//! is to reduce the amount of binaries in our workspace.
//!
//! Motivation: <https://github.com/rerun-io/rerun/issues/4623>

// TODO(#3408): remove unwrap()
#![allow(clippy::unwrap_used)]

use std::{fs, path::Path};

use itertools::Itertools as _;
use rust_format::Formatter as _;

fn main() {
    let crate_path =
        Path::new(&re_build_tools::get_and_track_env_var("CARGO_MANIFEST_DIR").unwrap()).to_owned();
    let all_path = crate_path.join("all");
    let src_path = crate_path.join("src");
    let snippets_path = src_path.join("snippets");

    assert!(
        all_path.exists() && all_path.is_dir(),
        "Failed to find {all_path:?}"
    );

    let mut snippets = Vec::new();

    re_build_tools::rerun_if_changed(&all_path);
    for subdir in fs::read_dir(&all_path).unwrap().flatten() {
        if !subdir.path().is_dir() {
            continue;
        }

        for entry in fs::read_dir(subdir.path()).unwrap().flatten() {
            let path = entry.path();
            if let Some(extension) = path.extension() {
                if extension == "rs" {
                    let snippet_name = path.file_stem().unwrap().to_str().unwrap().to_owned();

                    let contents = fs::read_to_string(&path).unwrap();

                    // TODO(#4047): some snippets lack a main, they should come with their necessary stub code commented out so that we can re-add it here.
                    if contents.contains("fn main()") {
                        // Patch the source code so we can call into `main` and pass arguments to it:
                        let contents =
                            contents.replace("fn main()", "pub fn main(_args: &[String])");
                        let contents = contents.replace(
                            "let args = std::env::args().collect::<Vec<_>>();",
                            "let args = _args;",
                        );
                        let contents = format!(
                            "//! DO NOT EDIT! This file was autogenerated by `{}`. The original is in `{}`.\n{contents}",
                            file!().replace('\\', "/"),
                            path.to_str().unwrap().replace('\\', "/"),
                        );

                        let target_path = snippets_path.join(format!("{snippet_name}.rs"));
                        println!("{}", target_path.display());
                        re_build_tools::write_file_if_necessary(target_path, contents.as_bytes())
                            .expect("failed to write snippet??");

                        snippets.push(snippet_name);
                    }
                } else if extension == "png" {
                    // Files used by the snippets, e.g. via `include_bytes`. Copy them:
                    let target_path = snippets_path.join(path.file_name().unwrap());
                    fs::copy(&path, &target_path).unwrap();
                }
            }
        }
    }

    assert!(
        snippets.len() > 10,
        "Found too few snippets in {all_path:?}"
    );

    let source = r#"
    //! DO NOT EDIT! Code generated by ${FILE}.

    #![allow(clippy::exit)]

    ${MODS}

    pub fn run() {
        let args: Vec<String> = std::env::args().skip(1).collect();

        if args.is_empty() {
            eprintln!("Usage: {} <snippet-name>", std::env::args().next().unwrap_or("snippets".to_owned()));
            eprintln!("Available snippets:");
            eprintln!();
            eprintln!("${SNIPPETS}");
            std::process::exit(1);
        }

        let snippet_name = args[0].as_str();

        match snippet_name {
            ${MATCH_SNIPPETS}
            _ => {
                eprintln!("Unknown snippet: {snippet_name}");
                eprintln!("Available snippets:");
                eprintln!();
                eprintln!("${SNIPPETS}");
                std::process::exit(1);
            }
        }
    }
    "#
    .trim()
    .replace("${FILE}", file!())
    .replace(
        "${MODS}",
        &snippets.iter().map(|m| format!("mod {m};")).join("\n"),
    )
    .replace("${SNIPPETS}", &snippets.iter().join("\\n"))
    .replace(
        "${MATCH_SNIPPETS}",
        &snippets
            .iter()
            .map(|m| {
                format!(
                    r#"{m:?} => {{
                        if let Err(err) = {m}::main(&args) {{
                            panic!("Failed to run '{m}': {{err}}");
                        }}
                    }}"#
                )
            })
            .join(",\n"),
    );

    let source = rust_format::RustFmt::default()
        .format_str(source)
        .expect("Failed to format");

    re_build_tools::write_file_if_necessary(snippets_path.join("mod.rs"), source.as_bytes())
        .unwrap();
}
