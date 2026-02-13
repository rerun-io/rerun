//! This module implements one half of our cross-platform #import system.
//!
//! The other half is provided as an extension to the build system, see the `build.rs` file
//! at the root of this crate.
//!
//! While it is agnostic to the type of files being imported, in practice this is only used
//! for shaders, thus this is what this documentation will linger on.
//! In particular, integration with our hot-reloading capabilities can get tricky depending
//! on the platform/target.
//!
//! ## Usage
//!
//! `#import <x/y/z/my_file.wgsl>`
//!
//! ### Syntax
//!
//! Import clauses follow the general form of `#import <x/y/z/my_file.wgsl>`.
//! The path to be imported can be either absolute or relative to the path of the importer,
//! or relative to any of the paths set in the search path (`RERUN_SHADER_PATH`).
//!
//! The actual parsing rules themselves are very barebones:
//! - An import clause can only span one line.
//! - An import clause line must start with `#import ` (exl. whitespaces).
//! - Everything between the first `<` and the last `>` is interpreted as the import
//!   path, as-is. We do so because, between the 4 major platforms (Linux, macOS, Window, Web),
//!   basically any string is a valid path.
//!
//! Everything is `trim()`ed at every step, you do not need to worry about whitespaces.
//!
//! ### Resolution
//!
//! Resolution is done in three steps:
//! 1. First, we try to interpret the imported path as absolute.
//!    1.1. If this is possible and leads to an existing file, we're done.
//!    1.2. Otherwise, we go to 2.
//!
//! 2. Second, we try to interpret the imported path as relative to the importer's.
//!    2.1. If this leads to an existing file, we're done.
//!    2.2. Otherwise, we go to 3.
//!
//! 3. Finally, we try to interpret the imported path as relative to all the directories
//!    present in the search path, in their prefined priority order, similar to e.g. how
//!    the standard `$PATH` environment variable behaves.
//!    3.1. If this leads to an existing file, we're done.
//!    3.2. Otherwise, resolution failed: throw an error.
//!
//! ### Interpolation
//!
//! Interpolation is done in the simplest way possible: the entire line containing the import
//! clause is overwritten with the contents of the imported file.
//! This is of course a recursive process.
//!
//! #### A word about `#pragma` semantics
//!
//! Imports can behave in two different ways: `#pragma once` and `#pragma many`.
//!
//! `#pragma once` means that each unique #import clause is only be resolved once even if it
//! used several times, e.g. assuming that `a.txt` contains the string `"xyz"` then:
//! ```raw
//! #import <a.txt>
//! #import <a.txt>
//! ```
//! becomes
//! ```raw
//! xyz
//! ```
//!
//! `#pragma many` on the other hand will resolve the clause as many times as it is used:
//! ```raw
//! #import <a.txt>
//! #import <a.txt>
//! ```
//! becomes
//! ```raw
//! xyz
//! xyz
//! ```
//!
//! At the moment, our import system only provides support for `#pragma once` semantics.
//!
//! ## Hot-reloading: platform specifics
//!
//! This import system transparently integrates with the renderer's hot-reloading capabilities.
//! What that actually means in practice depends on the platform/target.
//!
//! A general over-simplification of what we're aiming for can be expressed as:
//! > Be lazy in debug, be eager in release.
//!
//! When targeting native debug builds, we want everything to be as lazy as possible, everything
//! to happen just-in-time, e.g.:
//! - We always talk directly with the filesystem and check for missing files at the last moment.
//! - We do resolution & interpolation just-in-time, e.g. just before calling
//!   `create_shader_module`.
//! - Etc.
//!
//! On the web, we don't even have an actual filesystem to access at runtime, so not only we'd
//! like to be as eager can be, we don't have much of a choice to begin with.
//! That said, we don't want to be _too_ eager either: while we do have to make sure that every
//! single shader that we're gonna use (whether directly or indirectly via an import) ends up
//! in the final artifact one way or another, we still want to delay interpolation as much as
//! we can, otherwise we'd be bloating the binary artifact with N copies of the exact same
//! shader code.
//!
//! Still, we'd like to limit the number of differences between targets/platforms.
//! And indeed, the current implementation uses a virtual filesystem approach to effectively
//! remove any difference between how the different platforms behave at run-time.
//!
//! ### Debug builds (excl. web)
//!
//! Native debug builds are straightforward:
//! - We handle resolution & interpolation just-in-time (i.e. when fetching file contents).
//! - We always talk directly to the filesystem.
//!
//! No surprises there.
//!
//! ### Release builds (incl. web)
//!
//! Things are very different for release artifacts, as 1) we disable hot-reloading there and
//! 2) we never interact with the OS filesystem at run-time.
//! Still, in practice, we handle release builds just the same as debug ones.
//!
//! What happens there is we have a virtual, hermetic, in-memory filesystem that gets pre-loaded
//! with all the shaders defined within the Cargo workspace.
//! This happens in part through a build script that you can find at the root of this crate.
//!
//! From there, everything behaves exactly the same as usual. In fact, there is only one code
//! path for all platforms at run-time.
//!
//! There are many issues to deal with along the way though: paths comparisons across
//! environments and build-time/run-time, hermeticism, etc…
//! We won't cover those here: please refer to the code if you're curious.
//!
//! ## For developers
//!
//! ### Canonicalization vs. Normalization
//!
//! Comparing paths can get tricky, especially when juggling target environments and
//! run-time vs. compile-time constraints.
//! For this reason you'll see plenty mentions of canonicalization and normalization all over
//! the code: better make sure there's no confusion here.
//!
//! Canonicalization (i.e. `std::fs::canonicalize`) relies on syscalls to both normalize a path
//! (including following symlinks!) and make sure the file it references actually exist.
//!
//! It's the strictest form of path normalization you can get (and therefore ideal), but
//! requires 1) to have access to an actual filesystem at run-time and 2) that the file
//! being referenced already exists.
//!
//! Normalization (not available in `std`) on the other hand is purely lexicographical: it
//! normalizes paths as best as it can without ever touching the filesystem.
//!
//! See also "[Getting Dot-Dot Right](https://9p.io/sys/doc/lexnames.html)".
//!
//! ### Hermeticism
//!
//! When shipping release artifacts (whether web or otherwise), we want to avoid leaking state
//! from the original build environments into the final binary (think: paths, timestamps, etc).
//! We need to the build to be _hermetic_.
//!
//! Rust's `file!()` macro already takes care of that to some extent, and we need to match that
//! behavior on our side (e.g. by not leaking local paths), otherwise we won't be able to
//! compare paths at runtime.
//!
//! Think of it as `chroot`ing into our Cargo workspace :)
//!
//! In our case, there's an extra invariant on top on that: we must never embed shaders from
//! outside the workspace into our release artifacts!
//!
//! ## Things we don't support
//!
//! - Async: everything in this module is done using standard synchronous APIs.
//! - Compression, minification, etc: everything we embed is embedded as-is.
//! - Importing via network requests: only the (virtual) filesystem is supported for now.
//! - Implicit file suffixes: e.g. `#import <myshader>` for `myshader.wglsl`.
//! - Embedding raw Naga modules: not yet, though we have everything in place for it.

// TODO(cmc): might want to support implicitly dropping file suffixes at some point, e.g.
// `#import <my_shader>` which works with "my_shader.wgsl"

use std::path::{Path, PathBuf};
use std::rc::Rc;

use ahash::{HashMap, HashSet, HashSetExt as _};
use anyhow::{Context as _, anyhow, bail, ensure};
use clean_path::Clean as _;

use crate::FileSystem;

// ---

/// Specifies where to look for imports when both absolute and relative resolution fail.
///
/// This is akin to the standard `$PATH` environment variable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchPath {
    /// All directories currently in the search path, in decreasing order of priority.
    /// They are guaranteed to be normalized, but not canonicalized.
    dirs: Vec<PathBuf>,
}

impl SearchPath {
    pub fn from_env() -> Self {
        const RERUN_SHADER_PATH: &str = "RERUN_SHADER_PATH";

        std::env::var(RERUN_SHADER_PATH)
            .map_or_else(|_| Ok(Self::default()), |s| s.parse())
            .unwrap_or_else(|_| Self::default())
    }

    /// Push a path to search path.
    ///
    /// The path is normalized first, but not canonicalized.
    pub fn push(&mut self, dir: impl AsRef<Path>) {
        self.dirs.push(dir.as_ref().clean());
    }

    /// Insert a path into search path.
    ///
    /// The path is normalized first, but not canonicalized.
    pub fn insert(&mut self, index: usize, dir: impl AsRef<Path>) {
        self.dirs.insert(index, dir.as_ref().clean());
    }

    /// Returns an iterator over the directories in the search path, in decreasing
    /// order of priority.
    pub fn iter(&self) -> impl Iterator<Item = &Path> {
        self.dirs.iter().map(|p| p.as_path())
    }
}

impl std::str::FromStr for SearchPath {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Using implicit Vec<Result<_>> -> Result<Vec<_>> collection.
        let dirs: Result<Vec<PathBuf>, _> = s
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse()
                    .with_context(|| format!("couldn't parse {s:?} as PathBuf"))
            })
            .collect();

        // We cannot check whether these actually are directories, since they are not
        // guaranteed to even exist yet!
        // Similarly, we cannot canonicalize here, but we can at least normalize.

        dirs.map(|dirs| Self {
            dirs: dirs.into_iter().map(|dir| dir.clean()).collect(),
        })
    }
}

impl std::fmt::Display for SearchPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = self
            .dirs
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join(":");
        f.write_str(&s)
    }
}

// ---

// TODO(cmc): codespan errors?

/// A pre-parsed import clause, as in `#import <something>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportClause {
    /// The path being imported, as-is: neither canonicalized nor normalized.
    path: PathBuf,
}

impl ImportClause {
    pub const PREFIX: &'static str = "#import ";
}

impl<P: Into<PathBuf>> From<P> for ImportClause {
    fn from(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl std::str::FromStr for ImportClause {
    type Err = anyhow::Error;

    fn from_str(clause_str: &str) -> Result<Self, Self::Err> {
        let s = clause_str.trim();

        ensure!(
            s.starts_with(Self::PREFIX),
            "import clause must start with {prefix:?}, got {s:?}",
            prefix = Self::PREFIX,
        );
        let s = s.trim_start_matches(Self::PREFIX).trim();

        let rs = s.chars().rev().collect::<String>();

        let splits = s
            .find('<')
            .and_then(|i0| rs.find('>').map(|i1| (i0 + 1, rs.len() - i1 - 1)));

        if let Some((i0, i1)) = splits {
            let s = &s[i0..i1];
            ensure!(!s.is_empty(), "import clause must contain a non-empty path");

            return s
                .parse()
                .with_context(|| format!("couldn't parse {s:?} as PathBuf"))
                .map(|path| Self { path });
        }

        bail!("misformatted import clause: {clause_str:?}")
    }
}

impl std::fmt::Display for ImportClause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("#import <{}>", self.path.to_string_lossy()))
    }
}

#[cfg(test)]
mod tests_import_clause {
    use super::*;

    #[test]
    fn parsing_success() {
        let testcases: [(&str, PathBuf, Option<&str>); 16] = [
            (
                "#import <my_constants>",
                "my_constants".parse().unwrap(),
                None,
            ),
            (
                "#import <my_constants.wgsl>",
                "my_constants.wgsl".parse().unwrap(),
                None,
            ),
            (
                "#import <x/y/z/my_constants>",
                "x/y/z/my_constants".parse().unwrap(),
                None,
            ),
            (
                "#import <x/y/z/my_constants.wgsl>",
                "x/y/z/my_constants.wgsl".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my_constants>",
                "/x/y/z/my_constants".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my_constants.wgsl>",
                "/x/y/z/my_constants.wgsl".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my constants>",
                "/x/y/z/my constants".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my constants.wgsl>",
                "/x/y/z/my constants.wgsl".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my><constants>",
                "/x/y/z/my><constants".parse().unwrap(),
                None,
            ),
            (
                "#import </x/y/z/my><constants.wgsl>",
                "/x/y/z/my><constants.wgsl".parse().unwrap(),
                None,
            ),
            (
                "   #import \t\t\t   </x/y/z/my>\" \"<constants>       \t\t\t",
                "/x/y/z/my>\" \"<constants".parse().unwrap(),
                "#import </x/y/z/my>\" \"<constants>".into(),
            ),
            (
                "   #import \t\t\t   </x/y/z/my>\" \"<constants.wgsl>   \t\t\t",
                "/x/y/z/my>\" \"<constants.wgsl".parse().unwrap(),
                "#import </x/y/z/my>\" \"<constants.wgsl>".into(),
            ),
            // Non-sense, but a valid path nonetheless ¯\_(ツ)_/¯
            ("#import <<>>", "<>".parse().unwrap(), None),
            // Technically valid non-sense yet again!
            (
                "#import <my_constants.wgsl> <my_other_constants.wgsl>",
                "my_constants.wgsl> <my_other_constants.wgsl"
                    .parse()
                    .unwrap(),
                None,
            ),
            // Some more of that.
            (
                "#import <my_constants.wgsl> \t\t\t #import <my_other_constants.wgsl>",
                "my_constants.wgsl> \t\t\t #import <my_other_constants.wgsl"
                    .parse()
                    .unwrap(),
                None,
            ),
            // Going into "absolutely terrifying" territory
            (
                "#import <my_multiline\r\npath.wgsl>",
                "my_multiline\r\npath.wgsl".parse().unwrap(),
                None,
            ),
        ];
        let testcases = testcases
            .into_iter()
            .map(|(clause_str, path, clause_str_clean)| {
                (clause_str, ImportClause::from(path), clause_str_clean)
            });

        for (clause_str, expected, expected_clause) in testcases {
            eprintln!("test case: ({clause_str:?}, {expected:?})");

            let clause = clause_str.parse::<ImportClause>().unwrap();
            assert_eq!(expected, clause);

            let clause_str_clean = clause.to_string();
            if let Some(expected_clause) = expected_clause {
                assert_eq!(expected_clause, clause_str_clean);
            } else {
                assert_eq!(clause_str, clause_str_clean);
            }
        }
    }

    #[test]
    fn parsing_failure() {
        let testcases = [
            "#import <",
            "#import <>",
            "import my_constants",
            "my_constants",
        ];

        for s in testcases {
            eprintln!("test case: {s:?}");
            assert!(s.parse::<ImportClause>().is_err());
        }
    }
}

// ---

/// The recommended `FileResolver` type for the current platform/target.
#[cfg(load_shaders_from_disk)]
pub type RecommendedFileResolver = FileResolver<crate::OsFileSystem>;

/// The recommended `FileResolver` type for the current platform/target.
#[cfg(not(load_shaders_from_disk))]
pub type RecommendedFileResolver = FileResolver<&'static crate::MemFileSystem>;

/// Returns the recommended `FileResolver` for the current platform/target.
pub fn new_recommended() -> RecommendedFileResolver {
    let mut search_path = SearchPath::from_env();
    search_path.push("crates/viewer/re_renderer/shader");
    FileResolver::with_search_path(crate::get_filesystem(), search_path)
}

#[derive(Clone, Debug, Default)]
pub struct InterpolatedFile {
    pub contents: String,
    pub imports: HashSet<PathBuf>,
}

/// The `FileResolver` handles both resolving import clauses and doing the actual string
/// interpolation.
#[derive(Default)]
pub struct FileResolver<Fs> {
    /// A handle to the filesystem being used.
    /// Generally a `OsFileSystem` on native and a `MemFileSystem` on web and during tests.
    fs: Fs,

    /// The search path that we will go through when an import cannot be resolved neither
    /// as an absolute path or a relative one.
    search_path: SearchPath,
}

// Constructors
impl<Fs: FileSystem> FileResolver<Fs> {
    pub fn new(fs: Fs) -> Self {
        Self {
            fs,
            search_path: Default::default(),
        }
    }

    pub fn with_search_path(fs: Fs, search_path: SearchPath) -> Self {
        Self { fs, search_path }
    }
}

impl<Fs: FileSystem> FileResolver<Fs> {
    pub fn populate(&self, path: impl AsRef<Path>) -> anyhow::Result<InterpolatedFile> {
        re_tracing::profile_function!();

        fn populate_rec<Fs: FileSystem>(
            this: &FileResolver<Fs>,
            path: impl AsRef<Path>,
            interp_files: &mut HashMap<PathBuf, Rc<InterpolatedFile>>,
            path_stack: &mut Vec<PathBuf>,
            visited_stack: &mut HashSet<PathBuf>,
        ) -> anyhow::Result<Rc<InterpolatedFile>> {
            let path = path.as_ref().clean();

            // Cycle detection
            path_stack.push(path.clone());
            ensure!(
                visited_stack.insert(path.clone()),
                "import cycle detected: {path_stack:?}"
            );

            // #pragma once
            if interp_files.contains_key(&path) {
                // Cycle detection
                path_stack.pop().unwrap();
                visited_stack.remove(&path);

                return Ok(Default::default());
            }

            let contents = this.fs.read_to_string(&path)?;

            // Using implicit Vec<Result> -> Result<Vec> collection.
            let mut imports = HashSet::new();
            let children: Result<Vec<_>, _> = contents
                .lines()
                .map(|line| {
                    if line.trim().starts_with(ImportClause::PREFIX) {
                        let clause = line.parse::<ImportClause>()?;
                        // We do not use `Path::parent` on purpose!
                        let cwd = path.join("..").clean();
                        let clause_path =
                            this.resolve_clause_path(cwd, &clause.path).ok_or_else(|| {
                                anyhow!("couldn't resolve import clause path at {:?}", clause.path)
                            })?;
                        imports.insert(clause_path.clone());
                        populate_rec(this, clause_path, interp_files, path_stack, visited_stack)
                    } else {
                        // Fake child, just the line itself.
                        Ok(Rc::new(InterpolatedFile {
                            contents: line.to_owned(),
                            ..Default::default()
                        }))
                    }
                })
                .collect();
            let children = children?;

            let interp = children.into_iter().fold(
                InterpolatedFile {
                    imports,
                    ..Default::default()
                },
                |acc, child| InterpolatedFile {
                    contents: match (acc.contents.is_empty(), child.contents.is_empty()) {
                        (true, _) => child.contents.clone(),
                        (_, true) => acc.contents,
                        _ => [acc.contents.as_str(), child.contents.as_str()].join("\n"),
                    },
                    imports: acc.imports.union(&child.imports).cloned().collect(),
                },
            );

            let interp = Rc::new(interp);
            interp_files.insert(path.clone(), Rc::clone(&interp));

            // Cycle detection
            path_stack.pop().unwrap();
            visited_stack.remove(&path);

            Ok(interp)
        }

        let mut path_stack = Vec::new();
        let mut visited_stack = HashSet::new();
        let mut interp_files = HashMap::default();

        populate_rec(
            self,
            path,
            &mut interp_files,
            &mut path_stack,
            &mut visited_stack,
        )
        .map(|interp| (*interp).clone())
    }

    fn resolve_clause_path(
        &self,
        cwd: impl AsRef<Path>,
        path: impl AsRef<Path>,
    ) -> Option<PathBuf> {
        let path = path.as_ref().clean();

        // The imported path is absolute and points to an existing file, let's import that.
        if path.is_absolute() && self.fs.exists(&path) {
            return path.into();
        }

        // The imported path looks relative. Try to join it with the importer's and see if
        // that leads somewhere… if it does: import that.
        {
            let path = cwd.as_ref().join(&path).clean();
            if self.fs.exists(&path) {
                return path.into();
            }
        }

        // If the imported path isn't relative to the importer's, then maybe it is relative
        // with regards to one of the search paths: let's try there.
        for dir in self.search_path.iter() {
            let dir = dir.join(&path).clean();
            if self.fs.exists(&dir) {
                return dir.into();
            }
        }

        None
    }
}

// TODO(cmc): might want an actual test using `RERUN_SHADER_PATH`
#[cfg(test)]
mod tests_file_resolver {
    use unindent::unindent;

    use super::*;
    use crate::MemFileSystem;

    #[test]
    fn acyclic_interpolation() {
        let fs = MemFileSystem::get();
        {
            fs.create_dir_all("/shaders1/common").unwrap();
            fs.create_dir_all("/shaders1/a/b/c/d").unwrap();

            fs.create_file(
                "/shaders1/common/shader1.wgsl",
                unindent(
                    r#"
                    my first shader!
                    #import </shaders1/common/shader4.wgsl>
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders1/a/b/shader2.wgsl",
                unindent(
                    r#"
                    #import </shaders1/common/shader1.wgsl>
                    #import <../../common/shader1.wgsl>

                    #import </shaders1/a/b/c/d/shader3.wgsl>
                    #import <c/d/shader3.wgsl>

                    my second shader!

                    #import <common/shader1.wgsl>
                    #import <shader1.wgsl>

                    #import <shader3.wgsl>
                    #import <a/b/c/d/shader3.wgsl>
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders1/a/b/c/d/shader3.wgsl",
                unindent(
                    r#"
                    #import </shaders1/common/shader1.wgsl>
                    #import <../../../../common/shader1.wgsl>
                    my third shader!
                    #import <common/shader1.wgsl>
                    #import <shader1.wgsl>
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders1/common/shader4.wgsl",
                unindent(r#"my fourth shader!"#).into(),
            )
            .unwrap();
        }

        let resolver = FileResolver::with_search_path(fs, {
            let mut search_path = SearchPath::default();
            search_path.push("/shaders1");
            search_path.push("/shaders1/common");
            search_path.push("/shaders1/a/b/c/d");
            search_path
        });

        for _ in 0..3 {
            //   ^^^^  just making sure the stateful stuff behaves correctly

            let shader1_interp = resolver.populate("/shaders1/common/shader1.wgsl").unwrap();

            // Shader 1: resolve
            let mut imports = shader1_interp.imports.into_iter().collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec!["/shaders1/common/shader4.wgsl".into()];
            assert_eq!(expected, imports);

            // Shader 1: interpolate
            let contents = shader1_interp.contents;
            let expected = unindent(
                r#"
                my first shader!
                my fourth shader!"#,
            );
            assert_eq!(expected, contents);

            let shader2_interp = resolver.populate("/shaders1/a/b/shader2.wgsl").unwrap();
            // Shader 2: resolve
            let mut imports = shader2_interp.imports.into_iter().collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec![
                "/shaders1/a/b/c/d/shader3.wgsl".into(),
                "/shaders1/common/shader1.wgsl".into(),
                "/shaders1/common/shader4.wgsl".into(),
            ];
            assert_eq!(expected, imports);

            // Shader 2: interpolate
            let contents = shader2_interp.contents;
            let expected = unindent(
                r#"
                my first shader!
                my fourth shader!
                my third shader!
                my second shader!"#,
            );
            assert_eq!(expected, contents);

            let shader3_interp = resolver.populate("/shaders1/a/b/c/d/shader3.wgsl").unwrap();

            // Shader 3: resolve
            let mut imports = shader3_interp.imports.into_iter().collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec![
                "/shaders1/common/shader1.wgsl".into(),
                "/shaders1/common/shader4.wgsl".into(),
            ];
            assert_eq!(expected, imports);

            // Shader 3: interpolate
            let contents = shader3_interp.contents;
            let expected = unindent(
                r#"
                my first shader!
                my fourth shader!
                my third shader!"#,
            );
            assert_eq!(expected, contents);
        }
    }

    #[test]
    #[expect(clippy::should_panic_without_expect)] // TODO(cmc): check error contents
    #[should_panic]
    fn cyclic_direct() {
        let fs = MemFileSystem::get();
        {
            fs.create_dir_all("/shaders2").unwrap();

            fs.create_file(
                "/shaders2/shader1.wgsl",
                unindent(
                    r#"
                    #import </shaders2/shader2.wgsl>
                    my first shader!
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders2/shader2.wgsl",
                unindent(
                    r#"
                    #import </shaders2/shader1.wgsl>
                    my second shader!
                    "#,
                )
                .into(),
            )
            .unwrap();
        }

        let resolver = FileResolver::new(fs);

        resolver
            .populate("/shaders2/shader1.wgsl")
            .map_err(re_error::format)
            .unwrap();
    }

    #[test]
    #[expect(clippy::should_panic_without_expect)] // TODO(cmc): check error contents
    #[should_panic]
    fn cyclic_indirect() {
        let fs = MemFileSystem::get();
        {
            fs.create_dir_all("/shaders3").unwrap();

            fs.create_file(
                "/shaders3/shader1.wgsl",
                unindent(
                    r#"
                    #import </shaders3/shader2.wgsl>
                    my first shader!
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders3/shader2.wgsl",
                unindent(
                    r#"
                    #import </shaders3/shader3.wgsl>
                    my second shader!
                    "#,
                )
                .into(),
            )
            .unwrap();

            fs.create_file(
                "/shaders3/shader3.wgsl",
                unindent(
                    r#"
                    #import </shaders3/shader1.wgsl>
                    my third shader!
                    "#,
                )
                .into(),
            )
            .unwrap();
        }

        let resolver = FileResolver::new(fs);

        resolver
            .populate("/shaders3/shader1.wgsl")
            .map_err(re_error::format)
            .unwrap();
    }
}
