use std::collections::{BTreeMap, HashMap};
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

use super::{Context, DocumentData, DocumentKind};
use crate::build_search_index::util::{CommandExt as _, ProgressBarExt as _};

const RERUN_SDK: &str = "rerun_sdk";

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("python");

    // run `mkdocs` to generate documentation, which also produces a `objects.inv` file
    // this file contains every documented item and a URL to where it is documented
    progress.set("mkdocs build", ctx.is_tty());
    let mkdocs_config = ctx.workspace_root().join("rerun_py/mkdocs.yml");
    println!("Running mkdocs build with config: {mkdocs_config}");
    println!(
        "Current working directory: {:?}",
        std::env::current_dir().unwrap_or_default()
    );

    let mkdocs_result = Command::new("mkdocs")
        .with_arg("build")
        .with_arg("-f")
        .with_arg(&mkdocs_config)
        .output();

    match mkdocs_result {
        Ok(_) => println!("mkdocs build completed successfully"),
        Err(err) => {
            eprintln!("Failed to run mkdocs build:");
            eprintln!("  Error: {err}");
            eprintln!("  Config file: {mkdocs_config}");
            eprintln!("  Config file exists: {}", mkdocs_config.exists());
            if let Ok(cwd) = std::env::current_dir() {
                eprintln!("  Working directory: {cwd:?}");
            }
            return Err(err);
        }
    }

    // run `sphobjinv` to convert the `objects.inv` file into JSON, and fully resolve all links/names
    progress.set("sphobjinv convert", ctx.is_tty());
    let objects_inv_path = ctx.workspace_root().join("rerun_py/site/objects.inv");
    println!("Looking for objects.inv at: {objects_inv_path}");
    println!("objects.inv exists: {}", objects_inv_path.exists());

    let inv: Inventory = Command::new("sphobjinv")
        .with_args(["convert", "json", "--expand"])
        .with_cwd(ctx.workspace_root())
        .with_arg("rerun_py/site/objects.inv")
        .with_arg("-")
        .parse_json::<SphinxObjectInv>()
        .context(
            "sphobjinv may not be installed, try running `pixi run pip install -r rerun_py/requirements-doc.txt`",
        )?
        .objects
        .into_values()
        .map(|o| (o.name.clone(), o))
        .collect();

    // run `griffe` to obtain an tree of the entire public module hierarchy in `rerun_sdk`
    // this dump is only used to obtain docstrings
    progress.set("griffe dump", ctx.is_tty());
    let dump: Dump = Command::new("griffe")
        .with_args(["dump", "rerun_sdk", "-s", "rerun_py"])
        .parse_json()
        .context("either griffe or rerun_sdk is not installed, try running `pixi run pip install -r rerun_py/requirements-doc.txt` and building the SDK")?;

    let docs = collect_docstrings(&dump[RERUN_SDK]);

    // index each documented item
    let base_url = format!(
        "https://ref.rerun.io/docs/python/{version}",
        version = ctx.release_version()
    );
    // let base_url = "https://ref.rerun.io/docs/python/main";
    for (path, obj) in inv {
        ctx.push(DocumentData {
            kind: DocumentKind::Python,
            hidden_tags: vec!["py".into(), "python".into()],
            tags: vec![],
            content: docs.get(&path).cloned().unwrap_or_default(),
            url: format!("{base_url}/{uri}", uri = obj.uri),
            title: path,
        });
    }

    ctx.finish_progress_bar(progress);

    Ok(())
}

#[derive(Debug)]
struct Docstrings {
    /// `item_path -> docstring`
    docstrings: HashMap<String, String>,

    /// `alias_path -> item_path`
    ///
    /// Also includes the `item_path` itself
    aliases: HashMap<String, String>,
}

impl Docstrings {
    fn get(&self, path: &str) -> Option<&String> {
        // try get the docstring directly
        if let Some(v) = self.docstrings.get(path) {
            return Some(v);
        }

        // if `path` is an alias, get the qualified path
        let path = self.aliases.get(path)?;

        self.docstrings.get(path)
    }
}

fn collect_docstrings(root: &Item) -> Docstrings {
    #[derive(Default)]
    struct CollectDocstrings {
        /// `item_path -> docstring`
        docstrings: HashMap<String, String>,

        /// `alias_path -> item_path`
        ///
        /// Also includes `item_path -> item_path`
        aliases: HashMap<String, String>,
        module_path: Vec<String>,
    }

    impl CollectDocstrings {
        fn qualified_path(&self, item_name: &str) -> String {
            let mut path = self.module_path.join(".");
            path.push('.');
            path.push_str(item_name);
            path
        }
    }

    impl Visitor for CollectDocstrings {
        fn visit_module(&mut self, module: &Module) {
            let qpath = self.qualified_path(&module.name);
            if let Some(docstring) = &module.docstring {
                self.docstrings
                    .insert(qpath.clone(), docstring.value.clone());
            }
            self.aliases.insert(qpath.clone(), qpath);

            self.module_path.push(module.name.clone());
            module.visit(self);
            self.module_path.pop();
        }

        fn visit_alias(&mut self, alias: &Alias) {
            let qpath = self.qualified_path(&alias.name);
            self.aliases.insert(qpath, alias.target_path.clone());

            alias.visit(self);
        }

        fn visit_attribute(&mut self, attribute: &Attribute) {
            let qpath = self.qualified_path(&attribute.name);
            if let Some(docstring) = &attribute.docstring {
                self.docstrings
                    .insert(qpath.clone(), docstring.value.clone());
            }
            self.aliases.insert(qpath.clone(), qpath);

            attribute.visit(self);
        }

        fn visit_function(&mut self, function: &Function) {
            let qpath = self.qualified_path(&function.name);
            if let Some(docstring) = &function.docstring {
                self.docstrings
                    .insert(qpath.clone(), docstring.value.clone());
            }
            self.aliases.insert(qpath.clone(), qpath);

            function.visit(self);
        }

        fn visit_class(&mut self, class: &Class) {
            let qpath = self.qualified_path(&class.name);
            if let Some(docstring) = &class.docstring {
                self.docstrings
                    .insert(qpath.clone(), docstring.value.clone());
            }
            self.aliases.insert(qpath.clone(), qpath);

            class.visit(self);
        }
    }

    let mut visitor = CollectDocstrings::default();

    let Item::Module(root) = root else {
        panic!("root must be a module");
    };

    for member in root.members.values() {
        member.visit(&mut visitor);
    }

    Docstrings {
        docstrings: visitor.docstrings,
        aliases: visitor.aliases,
    }
}

#[expect(dead_code)] // unused fields exist only to make deserialization work
#[derive(Debug, Deserialize)]
struct SphinxObjectInv {
    // load-bearing fields: do not remove!
    project: String,
    version: String,
    count: usize,

    /// Keys in this hashmap are the index of the object,
    /// not the name/path.
    #[serde(flatten)]
    objects: HashMap<String, Object>,
}

#[derive(Debug, Deserialize)]
struct Object {
    name: String,
    uri: String,
}

type Inventory = HashMap<String, Object>;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "kind")]
enum Item {
    Module(Box<Module>),
    Alias(Box<Alias>),
    Attribute(Box<Attribute>),
    Function(Box<Function>),
    Class(Box<Class>),
}

#[derive(Debug, Deserialize)]
struct Module {
    name: String,
    // labels: Vec<String>,
    members: BTreeMap<String, Item>,
    docstring: Option<Docstring>,
}

#[derive(Debug, Deserialize)]
struct Alias {
    name: String,
    target_path: String,
}

#[derive(Debug, Deserialize)]
struct Attribute {
    name: String,
    // labels: HashSet<Label>,
    docstring: Option<Docstring>,
}

#[derive(Debug, Deserialize)]
struct Function {
    name: String,
    docstring: Option<Docstring>,
}

#[derive(Clone, Debug, Deserialize)]
struct Docstring {
    value: String,
}

#[derive(Debug, Deserialize)]
struct Class {
    name: String,
    docstring: Option<Docstring>,
    members: BTreeMap<String, Item>,
}

type Dump = BTreeMap<String, Item>;

trait Visitor {
    #[inline]
    fn visit_module(&mut self, module: &Module) {
        module.visit(self);
    }

    #[inline]
    fn visit_alias(&mut self, alias: &Alias) {
        alias.visit(self);
    }

    #[inline]
    fn visit_attribute(&mut self, attribute: &Attribute) {
        attribute.visit(self);
    }

    #[inline]
    fn visit_function(&mut self, function: &Function) {
        function.visit(self);
    }

    #[inline]
    fn visit_class(&mut self, class: &Class) {
        class.visit(self);
    }
}

trait Visit {
    fn visit<T: ?Sized + Visitor>(&self, visitor: &mut T);
}

impl Visit for Item {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, visitor: &mut T) {
        match self {
            Self::Module(v) => visitor.visit_module(v),
            Self::Alias(v) => visitor.visit_alias(v),
            Self::Attribute(v) => visitor.visit_attribute(v),
            Self::Function(v) => visitor.visit_function(v),
            Self::Class(v) => visitor.visit_class(v),
        }
    }
}

impl Visit for Module {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, visitor: &mut T) {
        for member in self.members.values() {
            member.visit(visitor);
        }
    }
}

impl Visit for Alias {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, _visitor: &mut T) {}
}

impl Visit for Attribute {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, _visitor: &mut T) {}
}

impl Visit for Function {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, _visitor: &mut T) {}
}

impl Visit for Class {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, visitor: &mut T) {
        for member in self.members.values() {
            member.visit(visitor);
        }
    }
}
