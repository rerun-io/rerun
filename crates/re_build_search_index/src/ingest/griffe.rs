use std::collections::BTreeMap;
use std::collections::HashSet;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

use serde::Deserialize;

use crate::ingest::DocumentData;

use super::Context;

const RERUN_SDK: &str = "rerun_sdk";

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("griffe (generating json)");
    let dump = griffe(ctx.workspace_root(), RERUN_SDK)?;
    let root = &dump[RERUN_SDK];
    collect_items(ctx, root);
    Ok(())
}

fn griffe(cwd: impl AsRef<Path>, pkg: &str) -> anyhow::Result<Dump> {
    let mut cmd = Command::new("griffe");
    cmd.args(["dump", pkg]);
    cmd.stdout(Stdio::piped());
    cmd.current_dir(cwd);
    let mut cmd = cmd.spawn()?;
    let stdout = cmd.stdout.as_mut().unwrap();
    let reader = BufReader::new(stdout);
    Ok(serde_json::from_reader(reader)?)
}

// Collect all modules, classes, and functions
fn collect_items(ctx: &Context, root: &Item) {
    struct CollectItems<'a> {
        ctx: &'a Context,
        path: Vec<String>,
        base_url: String,
    }

    impl CollectItems<'_> {
        fn push(&mut self) {
            self.ctx.push(DocumentData {
                kind: todo!(),
                title: todo!(),
                tags: vec![],
                content: todo!(),
                url: todo!(),
            })
        }
    }

    impl Visitor for CollectItems<'_> {
        fn visit_module(&mut self, module: &Module) {
            self.path.push(module.name.clone());
            module.visit(self);
            self.path.pop();
        }
    }

    let mut visitor = CollectItems {
        ctx,
        path: vec![],
        base_url: todo!(),
    };
    root.visit(&mut visitor);
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
    labels: Vec<String>,
    members: Vec<Item>,
}

#[derive(Debug, Deserialize)]
struct Alias {
    name: String,
    target_path: String,
}

#[derive(Debug, Deserialize)]
struct Attribute {
    name: String,
    labels: HashSet<Label>,
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

impl Function {
    fn docs(&self) -> Option<&str> {
        self.docstring.as_ref().map(|v| v.value.as_str())
    }
}

#[derive(Debug, Deserialize)]
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
