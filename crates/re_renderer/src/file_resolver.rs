// Let's talk about imports:
// - simple absolute & relative path imports
// - PATH-like imports
//
// Syntax:
// - imports sit on one line, no more, no less
// - `#import <my_constants>`
// - `#import <my_constants.wgsl>`
// - `#import <x/y/z/my_constants>"`
// - `#import <x/y/z/my_constants.wgsl>`
//
// First, we try the path as-is, assuming CWD is the path of the file where the import clause
// was found.
//
// Second, we try the path as part of a search path.

// LAZY ON NATIVE, (VERY) EAGER ON WEB
// ===================================
//
// On native, we want everything to be as lazy as possible, everything is just-in-time:
// - we always talk directly with the filesystem
//    - we check for missing file at the last minute
//    - this mean one can create new files even while the system is running and start
//      referencing them through other existing (or not) files
// - we do the import-stitching just-in-time, i.e. just before calling create_shader_module
//
//
// On the web, we don't even have a filesystem at runtime, so not only we'd like to be eager,
// we don't have much of a choice.
// That said, we don't want to be _too_ eager: while we do have to make sure that every single
// shader that we're gonna use (whether directly or indirectly via an import) ends up in the
// final artifact one way or another, we still want to delay the stitching as much as we can,
// otherwise we'd be wasting a lot of space for duplicated shader data.
//
// Keep in mind that for now we completely punt the issue of grabbing shaders via HTTP requests.
//
// Questions:
// - Shall one just gzip the embedded shader data at some point?

// CURRENT PLAN
// ============
//
// Native
// ------
//
// Everything is as lazy as it can be.
//
// Loading a file through the `FileServer` is the exact same as today: it does nothing other
// than instantiate a `FileContentsHandle::Path(path)`.
//
// Resolving the contents of a `FileContentsHandle` is where things become interesting:
// 1. We read the file as-is.
// 2. We parse the contents in search of root `ImportClause`s.
// 3. We recurse as needed, going through 1) and 2) again and again until hitting a leaf
//     1. Make sure to catch import cycles!
//     2. At this point we can finally canonicalize all these paths.
//     3. Some of these clauses might point to non-existing files etc.
// 4. We do the actual stitching, starting with the leaves until we hit the root.
// 5. We're done, pass the result to `create_shader_module`.
//
// Web
// ---
//
// Everything is as eager as it can be, except for the stitching.
// The stitching still happens just-in-time because we don't want to bloat the binary artifact
// with N copies of the exact same shader code.
//
//
// `include_file!` behaves very differently in this case: rather than just inlining the shader
// code into the binary, we generate code that will copy the shader's contents into a runtime
// hashmap that will act as a kind of virtual filesystem (remember: we don't have any filesystem
// on the web for now and not for the foreseeable future!).
// Think of it as `HashMap<OriginalPath, InlinedContents>`.
//
// Resolving the contents of a `FileContentsHandle` then becomes almost identical to the
// execution path taken on native:
// 1. We "read" the inlined contents from the virtual filesystem hashmap
// 2. We parse the contents in search of root `ImportClause`s.
// 3. We recurse as needed, going through 1) and 2) again and again until hitting a leaf
//     1. Make sure to catch import cycles!
//     2. All paths are guaranteed to point to existing virtual files [*]
// 4. We do the actual stitching, starting with the leaves until we hit the root.
// 5. We're done, pass the result to `create_shader_module`.
//
// [*] Well that is not exactly true, but it could be: we have all the info we need to make
// sure all import clauses that have been inlined are correct (and acyclic!), ahead of time.
//
//
// That way we keep things relatively simple and similar on both platforms for now; and much
// later on we can add a separate build path to add support for pre-compiled naga modules and
// such if we feel there's a need for it.
// Also what I like about this is that this doens't completely shut the door on the idea of
// importing stuff through URLs, which can be very valuable in some scenarios (both for us
// and end users) I feel like.

// TODO: explain canonicalization vs. normalization

use std::path::{Path, PathBuf};

use ahash::{HashMap, HashSet, HashSetExt};
use anyhow::{anyhow, bail, ensure, Context as _};
use clean_path::Clean as _;

// ---

// TODO: what do we do about async-ness? most likely we ignore it for now
// TODO: what do we do about network requests? most likely we ignore it for now
// TODO: what do we do about compression? most likely we ignore it for now
// TODO: one probably wants to trim() everything before inlining on web?

// NOTE: Paths used in search trees aren't canonicalized (they can't be: the destinations
// don't even have to exist yet), which means that one should be careful when comparing them.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchPath {
    /// All paths currently in the search path, in decreasing order of priority.
    /// They are guaranteed to be normalized, but not canonicalized.
    dirs: Vec<PathBuf>,
}

impl SearchPath {
    // TODO: gotta think about the web tho
    pub fn from_env() -> anyhow::Result<Self> {
        const RERUN_SHADER_PATH: &str = "RERUN_SHADER_PATH";
        const CARGO_MANIFEST_DIR: &str = "CARGO_MANIFEST_DIR";

        let this = std::env::var(RERUN_SHADER_PATH)
            .map_or_else(|_| Ok(SearchPath::default()), |s| s.parse())?;

        // TODO: default dirs when running from cargo
        if let Ok(s) = std::env::var(CARGO_MANIFEST_DIR) {}

        Ok(this)
    }

    /// Push a path to search path.
    ///
    /// The path is normalized, but not canonicalized.
    pub fn push(&mut self, path: impl AsRef<Path>) {
        self.dirs.push(path.as_ref().clean());
    }

    /// Insert a path into search path.
    ///
    /// The path is normalized, but not canonicalized.
    pub fn insert(&mut self, index: usize, path: impl AsRef<Path>) {
        self.dirs.insert(index, path.as_ref().clean());
    }

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

        // TODO: actually if we build the search-tree just in time then we're allowed
        // to canonicalize here?

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

// TODO: codespan error handling

// NOTE: Paths used in import clauses aren't canonicalized (they can't be: the destination
// doesn't even have to exist yet), which means that one should be careful when comparing them.

// TODO: this need a canonicalize method
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportClause {
    path: PathBuf,
}

impl<P: Into<PathBuf>> From<P> for ImportClause {
    fn from(path: P) -> Self {
        Self { path: path.into() }
    }
}

impl std::str::FromStr for ImportClause {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        const IMPORT_CLAUSE: &str = "#import ";
        ensure!(
            s.starts_with(IMPORT_CLAUSE),
            "import clause must start with '#import '"
        );
        let s = s.trim_start_matches(IMPORT_CLAUSE).trim();

        let rs = s.chars().rev().collect::<String>();

        let splits = s
            .find('<')
            .and_then(|i0| rs.find('>').map(|i1| (i0 + 1, rs.len() - i1 - 1)));

        if let Some((i0, i1)) = splits {
            let s = &s[i0..i1];
            ensure!(!s.is_empty(), "import clause must contain a non-empty path");

            return s
                .parse()
                .with_context(|| "couldn't parse {s:?} as PathBuf")
                .map(|path| Self { path });
        }

        bail!("misformatted import clause")
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
            // TODO(cmc): assert codespans?
        }
    }
}

// ---

// pub struct FileResolver {
// }

// impl FileResolver {
// }

// pub trait FileResolver {
//     fn resolve(path: impl AsRef<Path>) -> anyhow::Result<()>;
//     fn resolve_to_string(path: impl AsRef<Path>) -> anyhow::Result<String>;
// }

// TODO: search as vanilla path
// TODO: search PATH

// TODO: we do _NOT_ keep anything around, just do everything lazily

// TODO: put anyhow's contexts directly in there maybe?
pub trait FileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>>;
    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String>;
    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf>;
    fn exists(&self, path: impl AsRef<Path>) -> bool;

    fn create_dir_all(&mut self, _path: impl AsRef<Path>) -> anyhow::Result<()> {
        unimplemented!("not supported")
    }
    fn create_file(
        &mut self,
        _path: impl AsRef<Path>,
        _buf: impl AsRef<[u8]>,
    ) -> anyhow::Result<()> {
        unimplemented!("not supported")
    }
}

struct OsFileSystem;
impl FileSystem for OsFileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        let path = path.as_ref();
        std::fs::read(path).with_context(|| format!("failed to read file at {path:?}"))
    }

    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        let path = path.as_ref();
        std::fs::read_to_string(path).with_context(|| format!("failed to read file at {path:?}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref();
        std::fs::canonicalize(path)
            .with_context(|| format!("failed to canonicalize path at {path:?}"))
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        path.as_ref().exists()
    }
}

// TODO: need a Cow in there
#[derive(Default)]
struct MemFileSystem {
    files: HashMap<PathBuf, Vec<u8>>,
}
impl FileSystem for MemFileSystem {
    fn read(&self, path: impl AsRef<Path>) -> anyhow::Result<Vec<u8>> {
        let path = path.as_ref().clean();
        self.files
            .get(&path)
            .cloned()
            .ok_or_else(|| anyhow!("file does not exist"))
            .with_context(|| format!("failed to read file at {path:?}"))
    }

    fn read_to_string(&self, path: impl AsRef<Path>) -> anyhow::Result<String> {
        let path = path.as_ref().clean();
        let file = self
            .files
            .get(&path)
            .ok_or_else(|| anyhow!("file does not exist"))
            .with_context(|| format!("failed to read file at {path:?}"))?;
        String::from_utf8(file.clone()).with_context(|| format!("failed to read file at {path:?}"))
    }

    fn canonicalize(&self, path: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
        let path = path.as_ref().clean();
        self.files
            .get(&path)
            .ok_or_else(|| anyhow!("file does not exist at {path:?}"))
            .map(|_| path)
        // .with_context(|| format!("failed to canonicalize path at {path:?}"))
    }

    fn exists(&self, path: impl AsRef<Path>) -> bool {
        self.files.contains_key(&path.as_ref().clean())
    }

    fn create_dir_all(&mut self, _: impl AsRef<Path>) -> anyhow::Result<()> {
        Ok(())
    }

    fn create_file(&mut self, path: impl AsRef<Path>, buf: impl AsRef<[u8]>) -> anyhow::Result<()> {
        self.files
            .insert(path.as_ref().to_owned(), buf.as_ref().to_owned());
        Ok(())
    }
}

// ---

#[derive(Clone)]
struct InterpolatedFile {
    contents: String,
    imports: HashSet<PathBuf>,
}

// TODO: note how this cache files for its whole lifetime
#[derive(Default)]
struct FileResolver<Fs> {
    /// A handle to the filesystem being used.
    /// Generally a `OsFileSystem` on native and a `MemFileSystem` on web and during tests.
    fs: Fs,

    search_path: SearchPath, // TODO

    /// Already interpolated file cache, for the lifetime of this resolver.
    files: HashMap<PathBuf, InterpolatedFile>,
}

// Constructors
impl<Fs: FileSystem> FileResolver<Fs> {
    pub fn new(fs: Fs) -> Self {
        Self {
            fs,
            search_path: Default::default(),
            files: Default::default(),
        }
    }

    pub fn with_search_path(fs: Fs, search_path: SearchPath) -> Self {
        Self {
            fs,
            search_path,
            files: Default::default(),
        }
    }
}

// Public APIs: resolution & interpolation
impl<Fs: FileSystem> FileResolver<Fs> {
    pub fn resolve_contents(&mut self, path: impl AsRef<Path>) -> anyhow::Result<&str> {
        // puffin::profile_function!(); // TODO: puffin feature

        self.populate(&path)?;

        Ok(&self
            .files
            .get(path.as_ref())
            .expect("we've just populated!")
            .contents)
    }

    pub fn resolve_imports(
        &mut self,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<impl Iterator<Item = &Path>> {
        // puffin::profile_function!(); // TODO: puffin feature

        self.populate(&path)?;

        Ok(self
            .files
            .get(path.as_ref())
            .expect("we've just populated!")
            .imports
            .iter()
            .map(|p| p.as_path()))
    }
}

// Cache management
impl<Fs: FileSystem> FileResolver<Fs> {
    /// Populates the local, pre-interpolated cache.
    // TODO: lotta useless cloning going on
    fn populate(&mut self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        // puffin::profile_function!(); // TODO: puffin feature

        fn populate_rec<Fs: FileSystem>(
            this: &mut FileResolver<Fs>,
            path: impl AsRef<Path>,
            path_stack: &mut Vec<PathBuf>,
            visited: &mut HashSet<PathBuf>,
        ) -> anyhow::Result<String> {
            const IMPORT_CLAUSE: &str = "#import "; // TODO: pls dont dupe this

            let path = path.as_ref().clean();

            // Cycle detection
            path_stack.push(path.clone());
            ensure!(
                visited.insert(path.clone()),
                "import cycle detected: {path_stack:?}"
            );
            dbg!(&path); // TODO: might be worth a permanent re_debug!

            if !this.files.contains_key(&path) {
                let contents = this
                    .fs
                    .read_to_string(&path)
                    .with_context(|| format!("failed to read file at {path:?}"))?;

                // Using implicit Vec<Result> -> Result<Vec> collection.
                let mut imports = HashSet::new();
                let lines: Result<Vec<_>, _> = contents
                    .lines()
                    .map(|line| {
                        if line.trim().starts_with(IMPORT_CLAUSE) {
                            let clause = line.parse::<ImportClause>()?;
                            // We do not use `Path::parent` on purpose!
                            let cwd = path.join("..").clean();
                            let clause_path =
                                this.resolve_clause_path(cwd, &clause.path).ok_or_else(|| {
                                    anyhow!(
                                        "couldn't resolve import clause path at {:?}",
                                        clause.path
                                    )
                                })?;
                            imports.insert(clause_path.clone());
                            populate_rec(this, clause_path, path_stack, visited)
                        } else {
                            Ok(line.to_owned())
                        }
                    })
                    .collect();
                let lines = lines?;

                let contents = lines.join("\n");
                this.files.insert(
                    path.to_owned(),
                    InterpolatedFile {
                        contents: contents.clone(), // TODO
                        imports,
                    },
                );

                path_stack.pop().unwrap();
                visited.remove(&path);

                return Ok(contents);
            }

            // Cycle detection
            path_stack.pop().unwrap();
            visited.remove(&path);

            Ok(this.files.get(&path).unwrap().contents.clone()) // TODO: clone...
        }

        let mut path_stack = Vec::new();
        let mut visited = HashSet::new();
        populate_rec(self, path, &mut path_stack, &mut visited).map(|_| ())
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
        // that leads somewhere... if it does: import that.
        {
            let path = cwd.as_ref().join(&path).clean();
            if self.fs.exists(&path) {
                return path.into();
            }
        }

        // If the imported path isn't relative to the importer's, then maybe it is relative
        // with regards to one of the search paths: let's try there.
        for dir in self.search_path.iter() {
            let path = dir.join(&path).clean();
            if self.fs.exists(&path) {
                return path.into();
            }
        }

        None
    }
}

#[cfg(test)]
mod tests_file_resolver {
    use unindent::{unindent, unindent_bytes};

    use super::*;

    #[test]
    fn acyclic_interpolation() {
        let mut fs = MemFileSystem::default();
        {
            fs.create_dir_all("/shaders/common").unwrap();
            fs.create_dir_all("/shaders/a/b/c/d").unwrap();

            fs.create_file(
                "/shaders/common/shader1.wgsl",
                unindent_bytes(br#"my first shader!"#),
            )
            .unwrap();

            fs.create_file(
                "/shaders/a/b/shader2.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/common/shader1.wgsl>
                    #import <../../common/shader1.wgsl>

                    #import </shaders/a/b/c/d/shader3.wgsl>
                    #import <c/d/shader3.wgsl>

                    my second shader!

                    #import <common/shader1.wgsl>
                    #import <shader1.wgsl>

                    #import <shader3.wgsl>
                    #import <a/b/c/d/shader3.wgsl>
                    "#,
                ),
            )
            .unwrap();

            fs.create_file(
                "/shaders/a/b/c/d/shader3.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/common/shader1.wgsl>
                    #import <../../../../common/shader1.wgsl>
                    my third shader!
                    #import <common/shader1.wgsl>
                    #import <shader1.wgsl>
                    "#,
                ),
            )
            .unwrap();
        }

        let mut resolver = FileResolver::with_search_path(fs, {
            let mut search_path = SearchPath::default();
            search_path.push("/shaders");
            search_path.push("/shaders/common");
            search_path.push("/shaders/a/b/c/d");
            search_path
        });

        for _ in 0..3 {
            //   ^^^^  just making sure the stateful stuff behaves correctly

            // Shader 1: resolve
            let mut imports = resolver
                .resolve_imports("/shaders/common/shader1.wgsl")
                .unwrap()
                .collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec![];
            assert_eq!(expected, imports);

            // Shader 1: interpolate
            let contents = resolver
                .resolve_contents("/shaders/common/shader1.wgsl")
                .map_err(|err| re_error::format(err))
                .unwrap();
            let expected = unindent(r#"my first shader!"#);
            assert_eq!(expected, contents);

            // Shader 2: resolve
            let mut imports = resolver
                .resolve_imports("/shaders/a/b/shader2.wgsl")
                .unwrap()
                .collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec![
                "/shaders/a/b/c/d/shader3.wgsl".into(),
                "/shaders/common/shader1.wgsl".into(),
            ];
            assert_eq!(expected, imports);

            // Shader 2: interpolate
            let contents = resolver
                .resolve_contents("/shaders/a/b/shader2.wgsl")
                .map_err(|err| re_error::format(err))
                .unwrap();
            let expected = unindent(
                r#"
                my first shader!
                my first shader!

                my first shader!
                my first shader!
                my third shader!
                my first shader!
                my first shader!
                my first shader!
                my first shader!
                my third shader!
                my first shader!
                my first shader!

                my second shader!

                my first shader!
                my first shader!

                my first shader!
                my first shader!
                my third shader!
                my first shader!
                my first shader!
                my first shader!
                my first shader!
                my third shader!
                my first shader!
                my first shader!"#,
            );
            assert_eq!(expected, contents);

            // Shader 3: resolve
            let mut imports = resolver
                .resolve_imports("/shaders/a/b/c/d/shader3.wgsl")
                .unwrap()
                .collect::<Vec<_>>();
            imports.sort();
            let expected: Vec<PathBuf> = vec!["/shaders/common/shader1.wgsl".into()];
            assert_eq!(expected, imports);

            // Shader 3: interpolate
            let contents = resolver
                .resolve_contents("/shaders/a/b/c/d/shader3.wgsl")
                .map_err(|err| re_error::format(err))
                .unwrap();
            let expected = unindent(
                r#"
                my first shader!
                my first shader!
                my third shader!
                my first shader!
                my first shader!"#,
            );
            assert_eq!(expected, contents);
        }
    }

    #[test]
    #[should_panic] // TODO: check error contents
    fn cyclic_direct() {
        let mut fs = MemFileSystem::default();
        {
            fs.create_dir_all("/shaders").unwrap();

            fs.create_file(
                "/shaders/shader1.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/shader2.wgsl>
                    my first shader!
                    "#,
                ),
            )
            .unwrap();

            fs.create_file(
                "/shaders/shader2.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/shader1.wgsl>
                    my second shader!
                    "#,
                ),
            )
            .unwrap();
        }

        let mut resolver = FileResolver::new(fs);

        let contents = resolver
            .resolve_contents("/shaders/shader1.wgsl")
            .map_err(|err| re_error::format(err))
            .unwrap();
        let expected = unindent(r#"my first shader!"#);
        assert_eq!(expected, contents);
    }

    #[test]
    #[should_panic] // TODO: check error contents
    fn cyclic_indirect() {
        let mut fs = MemFileSystem::default();
        {
            fs.create_dir_all("/shaders").unwrap();

            fs.create_file(
                "/shaders/shader1.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/shader2.wgsl>
                    my first shader!
                    "#,
                ),
            )
            .unwrap();

            fs.create_file(
                "/shaders/shader2.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/shader3.wgsl>
                    my second shader!
                    "#,
                ),
            )
            .unwrap();

            fs.create_file(
                "/shaders/shader3.wgsl",
                unindent_bytes(
                    br#"
                    #import </shaders/shader1.wgsl>
                    my third shader!
                    "#,
                ),
            )
            .unwrap();
        }

        let mut resolver = FileResolver::new(fs);

        resolver
            .resolve_contents("/shaders/shader1.wgsl")
            .map_err(|err| re_error::format(err))
            .unwrap();
    }
}
