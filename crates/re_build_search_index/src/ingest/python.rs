use std::collections::BTreeMap;
use std::collections::HashMap;
use std::process::Command;

use anyhow::Context as _;
use serde::Deserialize;

use crate::ingest::DocumentData;
use crate::ingest::DocumentKind;
use crate::util::CommandExt as _;

use super::Context;

const RERUN_SDK: &str = "rerun_sdk";

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("python");

    // run `mkdocs` to generate documentation, which also produces a `objects.inv` file
    // this file contains every documented item and a URL to where it is documented
    progress.set_message("mkdocs build");
    progress.suspend(|| {
        Command::new("mkdocs")
            .with_arg("build")
            .with_arg("-f")
            .with_arg(ctx.workspace_root().join("rerun_py/mkdocs.yml"))
            .run_async()
    })?;

    // run `sphobjinv` to convert the `objects.inv` file into JSON, and fully resolve all links/names
    progress.set_message("sphobjinv convert");
    let inv: Inventory = Command::new("sphobjinv")
        .with_args(["convert", "json", "--expand"])
        .with_cwd(ctx.workspace_root())
        .with_arg("rerun_py/site/objects.inv")
        .with_arg("-")
        .run_serde::<SphinxObjectInv>()
        .context("sphobjinv may not be installed, install rerun_py/requirements-doc.txt")?
        .objects
        .into_values()
        .map(|o| (o.name.clone(), o))
        .collect();

    // run `griffe` to obtain an tree of the entire public module hierarchy in `rerun_sdk`
    // this dump is only used to obtain docstrings
    progress.set_message("griffe dump");
    let dump: Dump = Command::new("griffe")
        .with_args(["dump", "rerun_sdk"])
        .run_serde()
        .context("either griffe or rerun_sdk is not installed")?;

    let docs = collect_docstrings(&dump[RERUN_SDK]);

    // index each documented item
    let _version = &ctx.rerun_pkg().version;
    // let base_url = format!("https://ref.rerun.io/docs/python/{version}");
    let base_url = "https://ref.rerun.io/docs/python/main";
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
        let Some(path) = self.aliases.get(path) else {
            return None;
        };

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

    for member in &root.members {
        member.visit(&mut visitor);
    }

    Docstrings {
        docstrings: visitor.docstrings,
        aliases: visitor.aliases,
    }
}

#[allow(dead_code)] // unused fields exist only to make deserialization work
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
enum Role {
    Module,
    Attr,
    Function,
    Class,
}

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
    members: Vec<Item>,
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

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Label {
    #[serde(rename = "instance-attribute")]
    InstanceAttribute,
    #[serde(rename = "class-attribute")]
    ClassAttribute,
    #[serde(rename = "module-attribute")]
    ModuleAttribute,

    #[serde(other)]
    Unknown,
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
    members: Vec<Item>,
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
            Item::Module(v) => visitor.visit_module(v),
            Item::Alias(v) => visitor.visit_alias(v),
            Item::Attribute(v) => visitor.visit_attribute(v),
            Item::Function(v) => visitor.visit_function(v),
            Item::Class(v) => visitor.visit_class(v),
        }
    }
}

impl Visit for Module {
    #[inline]
    fn visit<T: ?Sized + Visitor>(&self, visitor: &mut T) {
        for member in &self.members {
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
        for member in &self.members {
            member.visit(visitor);
        }
    }
}
