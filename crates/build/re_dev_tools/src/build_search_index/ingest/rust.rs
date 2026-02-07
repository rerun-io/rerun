#![expect(clippy::unwrap_used)] // build tool, so okay here

use std::collections::HashSet;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;

use anyhow::Context as _;
use cargo_metadata::semver::Version;
use crossbeam::channel::Sender;
use indicatif::ProgressBar;
use rayon::prelude::{IntoParallelIterator as _, ParallelIterator as _};
use rustdoc_types::{Crate, Id as ItemId, Impl, Item, ItemEnum, Type, Use};

use super::{Context, DocumentData, DocumentKind};
use crate::build_search_index::util::ProgressBarExt as _;

/// Ingest rust documentation for all published crates in the current workspace.
///
/// It collects the following top-level `pub` items:
/// - `mod`
/// - `fn`
/// - `struct`
/// - `enum`
/// - `trait`
/// - `const`
/// - `type`
/// - `#[macro_export] macro_rules!`
///
/// In `impl` blocks, it collects:
/// - associated `const`
/// - associated `type`
/// - associated `fn`
///
/// It will also walk through any `pub mod`, and correctly resolve `pub use mod::item` where `mod` is not `pub`.
pub fn ingest(
    ctx: &Context,
    exclude_crates: &[String],
    rust_toolchain: &str,
) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("rustdoc");

    let mut crates = Vec::new();

    for pkg in ctx.metadata.workspace_packages() {
        progress.set(pkg.name.to_string(), ctx.is_tty());

        if exclude_crates.contains(&pkg.name) {
            continue;
        }

        let publish = match pkg.publish.as_deref() {
            Some([]) => false,      // explicitly set to `false`
            Some(_) | None => true, // omitted, set to `true`, or set to specific registry
        };
        if !publish {
            continue;
        }

        let is_library = pkg
            .manifest_path
            .parent()
            .unwrap()
            .join("src/lib.rs")
            .try_exists()?;
        if !is_library {
            continue;
        }

        let path = rustdoc_json::Builder::default()
            .toolchain(rust_toolchain)
            .all_features(true)
            .quiet(true)
            .manifest_path(&pkg.manifest_path)
            .build()?;

        let file = File::open(&path)
            .with_context(|| format!("reading {}", path.display()))
            .unwrap();
        let reader = BufReader::new(file);
        let krate: Crate = serde_json::from_reader(reader).unwrap();
        crates.push(krate);
    }

    let (tx, rx) = crossbeam::channel::bounded(1024);
    let version = ctx.release_version();

    ctx.finish_progress_bar(progress);

    crates
        .into_iter()
        .map(|krate| {
            (
                ctx.progress_bar(format!("rustdoc ({})", krate.name())),
                krate,
            )
        })
        .collect::<Vec<_>>()
        .into_par_iter()
        .for_each(|(progress, krate)| {
            let mut visitor = Visitor::new(progress, version, &tx, &krate);
            visitor.visit_root();
            visitor.progress.finish_and_clear();
        });

    drop(tx);
    for data in rx {
        ctx.push(data);
    }

    Ok(())
}

struct Visitor<'a> {
    progress: ProgressBar,
    visited: HashSet<ItemId>,
    documents: &'a Sender<DocumentData>,
    module_path: Vec<String>,
    krate: &'a Crate,
    base_url: String,
}

impl<'a> Visitor<'a> {
    fn new(
        progress: ProgressBar,
        version: &Version,
        documents: &'a Sender<DocumentData>,
        krate: &'a Crate,
    ) -> Self {
        let crate_name = krate.name();

        Self {
            progress,
            visited: HashSet::new(),
            documents,
            krate,
            module_path: vec![crate_name],
            base_url: base_url(version, krate),
        }
    }

    fn push(&mut self, pub_in_priv: bool, id: &ItemId, kind: ItemKind) {
        let path = self.resolve_path(pub_in_priv, id);
        self.push_with_path(id, kind, &path);
    }

    fn push_with_path(&mut self, id: &ItemId, kind: ItemKind, path: &[String]) {
        // don't push the same document twice
        if self.visited.contains(id) {
            return;
        }
        self.visited.insert(*id);

        let mut module_path = &path[..path.len() - 1];
        let name = path.last().unwrap();
        let item_path = match &kind {
            ItemKind::Module => {
                format!("{name}/index.html")
            }
            ItemKind::Struct
            | ItemKind::Enum
            | ItemKind::Trait
            | ItemKind::Function
            | ItemKind::Type
            | ItemKind::Constant
            | ItemKind::Macro => {
                format!("{kind}.{name}.html")
            }
            ItemKind::Inherent(parent, _) => {
                let parent_name = module_path.last().unwrap();
                module_path = &module_path[..module_path.len() - 1];
                format!("{parent}.{parent_name}.html#{kind}.{name}")
            }
        };

        self.documents
            .send(document(
                path.join("::"),
                format!("{}/{}/{}", self.base_url, module_path.join("/"), item_path),
                self.krate.index[id].docs.clone().unwrap_or_default(),
            ))
            .ok();
    }

    fn visit_root(&mut self) {
        let root_module_item = &self.krate.index[&self.krate.root];

        let ItemEnum::Module(root_module) = &root_module_item.inner else {
            unreachable!()
        };

        let name = root_module_item.name.as_ref().unwrap().clone();
        let url = format!("{}/{name}/index.html", self.base_url);
        self.documents
            .send(document(
                name.clone(),
                url,
                root_module_item.docs.clone().unwrap_or_default(),
            ))
            .ok();

        for item_id in &root_module.items {
            self.visit_item(false, item_id);
        }
    }

    fn visit_item(&mut self, pub_in_priv: bool, id: &ItemId) {
        let Some(item) = self.krate.index.get(id) else {
            panic!("{id:?} not found");
        };

        if item.crate_id != self.krate.index[&self.krate.root].crate_id {
            // skip items from external crates
            return;
        }

        use ItemEnum as I;
        match &item.inner {
            I::Module(inner) => {
                self.push(pub_in_priv, id, ItemKind::Module);
                let name = item.name.as_ref().unwrap().clone();

                self.module_path.push(name);
                for item_id in &inner.items {
                    self.visit_item(pub_in_priv, item_id);
                }
                self.module_path.pop();
            }
            I::Use(import) => self.visit_import(import),
            I::Impl(impl_) => {
                // we only care about inherent impls of the form:
                //   impl Thing {}
                let Some(type_id) = impl_.inherent_impl_type_id() else {
                    return;
                };
                let type_ = &self.krate.index[type_id];
                let type_kind = type_.kind().unwrap();
                let parent_kind = match type_kind {
                    ItemKind::Struct => ParentItemKind::Struct,
                    ItemKind::Enum => ParentItemKind::Enum,
                    ItemKind::Trait => ParentItemKind::Trait,
                    _ => return,
                };
                self.visit_inherent_impl(pub_in_priv, id, impl_, parent_kind);
            }
            I::Struct(struct_) => {
                self.push(pub_in_priv, id, ItemKind::Struct);
                for impl_id in &struct_.impls {
                    let ItemEnum::Impl(impl_) = &self.krate.index[impl_id].inner else {
                        panic!("invalid item {impl_id:?} expected `impl`, got {item:#?}");
                    };
                    if !impl_.is_inherent() {
                        continue;
                    }
                    self.visit_inherent_impl(pub_in_priv, id, impl_, ParentItemKind::Struct);
                }
            }
            I::Enum(enum_) => {
                self.push(pub_in_priv, id, ItemKind::Enum);
                for impl_id in &enum_.impls {
                    let ItemEnum::Impl(impl_) = &self.krate.index[impl_id].inner else {
                        panic!("invalid item {impl_id:?} expected `impl`, got {item:#?}");
                    };
                    if !impl_.is_inherent() {
                        continue;
                    }
                    self.visit_inherent_impl(pub_in_priv, id, impl_, ParentItemKind::Enum);
                }
            }
            I::Trait(trait_) => {
                self.push(pub_in_priv, id, ItemKind::Trait);
                for item_id in &trait_.items {
                    self.visit_assoc_item(pub_in_priv, id, item_id, ParentItemKind::Trait);
                }
            }
            I::Function(_) => self.push(pub_in_priv, id, ItemKind::Function),
            I::TypeAlias(_) => self.push(pub_in_priv, id, ItemKind::Type),
            I::Constant { .. } => self.push(pub_in_priv, id, ItemKind::Constant),
            I::Macro(_) => self.push(pub_in_priv, id, ItemKind::Macro),

            I::AssocConst { .. }
            | I::AssocType { .. }
            | I::Variant(_)
            | I::StructField(_)
            | I::Union(_)
            | I::ExternCrate { .. }
            | I::TraitAlias(_)
            | I::Static(_)
            | I::ExternType
            | I::ProcMacro(_)
            | I::Primitive(_) => {}
        }
    }

    fn visit_import(&mut self, import: &Use) {
        let Some(id) = import.id.as_ref() else {
            return;
        };

        if !self.krate.index.contains_key(id) {
            // this is an external crate
            return;
        }

        // NOTE: this currently relies on the following bug:
        // https://github.com/rust-lang/rust/issues/110007
        let is_pub = self.krate.paths.contains_key(id);
        if is_pub {
            // it already has a path consisting of `pub` modules
            // so it will be included even if we don't re-export it
            return;
        }

        let item = &self.krate.index[id];
        if import.is_glob {
            let ItemEnum::Module(module) = &item.inner else {
                unreachable!()
            };
            for item_id in &module.items {
                self.visit_item(true, item_id);
            }
        } else {
            self.visit_item(true, id);
        }
    }

    fn visit_inherent_impl(
        &mut self,
        pub_in_priv: bool,
        type_id: &ItemId,
        impl_: &Impl,
        parent_kind: ParentItemKind,
    ) {
        assert!(impl_.is_inherent());

        for item_id in &impl_.items {
            self.visit_assoc_item(pub_in_priv, type_id, item_id, parent_kind);
        }
    }

    fn visit_assoc_item(
        &mut self,
        pub_in_priv: bool,
        type_id: &ItemId,
        id: &ItemId,
        parent_kind: ParentItemKind,
    ) {
        let item = &self.krate.index[id];
        let kind = match &item.inner {
            ItemEnum::Function(_) => ItemKind::Inherent(parent_kind, InherentItemKind::Method),
            ItemEnum::AssocConst { .. } => {
                ItemKind::Inherent(parent_kind, InherentItemKind::Constant)
            }
            ItemEnum::AssocType { .. } => ItemKind::Inherent(parent_kind, InherentItemKind::Type),
            _ => unreachable!("invalid associated item {item:#?}"),
        };

        let name = item.name.as_ref().unwrap().clone();
        let path = self.resolve_path(pub_in_priv, type_id).with_item(name);
        self.push_with_path(id, kind, &path);
    }

    fn resolve_path(&self, pub_in_priv: bool, id: &ItemId) -> Vec<String> {
        if pub_in_priv {
            let name = self.krate.index[id].name.as_ref().unwrap().clone();
            self.module_path.with_item(name)
        } else {
            let Some(summary) = self.krate.paths.get(id) else {
                panic!(
                    "expected item {id:?} to have a rustdoc-generated path (module_path={:?})",
                    self.module_path
                );
            };
            summary.path.clone()
        }
    }
}

fn base_url(version: &Version, krate: &Crate) -> String {
    format!(
        "https://docs.rs/{krate_name}/{version}",
        krate_name = krate.name()
    )
    // format!("https://docs.rs/{}/latest", krate.name())
}

fn document(path: String, url: String, docs: String) -> DocumentData {
    DocumentData {
        kind: DocumentKind::Rust,
        title: path,
        hidden_tags: vec!["rust".into()],
        tags: vec![],
        content: docs,
        url,
    }
}

#[derive(Debug, Clone, Copy)]
enum ItemKind {
    /// `mod m`
    Module,

    /// `struct S {}`
    Struct,

    /// `enum E {}`
    Enum,

    /// `trait I {}`
    Trait,

    /// `fn f() {}`
    Function,

    /// `type T = ()`
    Type,

    /// `const V: T = ()`
    Constant,

    /// `macro_rules! m {}`
    Macro,

    /// Inherent impl item
    ///
    /// These are also referred to as "associated items"
    Inherent(ParentItemKind, InherentItemKind),
}

impl Display for ItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Module => "module",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Function => "fn",
            Self::Type => "type",
            Self::Constant => "constant",
            Self::Macro => "macro",
            Self::Inherent(ParentItemKind::Trait, InherentItemKind::Method) => "tymethod",
            Self::Inherent(_, InherentItemKind::Method) => "method",
            Self::Inherent(_, InherentItemKind::Constant) => "associatedconstant",
            Self::Inherent(_, InherentItemKind::Type) => "associatedtype",
        };
        f.write_str(s)
    }
}

/// `ItemKind` for items in inherent impls
///
/// These are also referred to as associated items
#[derive(Debug, Clone, Copy)]
enum InherentItemKind {
    /// A `fn` in an inherent `impl` block:
    ///
    /// ```rust,ignore
    /// struct T;
    ///
    /// impl T {
    ///     fn f() {} //<-
    /// }
    /// ```
    Method,

    /// A `const` in an inherent `impl` block:
    ///
    /// ```rust,ignore
    /// struct T;
    ///
    /// impl T {
    ///     const V: () = (); //<-
    /// }
    /// ```
    Constant,

    /// A `type` in an inherent `impl` block:
    ///
    /// ```rust,ignore
    /// struct T;
    ///
    /// impl T {
    ///     type U = (); //<-
    /// }
    /// ```
    Type,
}

/// `ItemKind` for types which may have inherent impls
#[derive(Debug, Clone, Copy)]
enum ParentItemKind {
    Struct,
    Enum,
    Trait,
}

impl Display for ParentItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
        };
        f.write_str(s)
    }
}

trait CrateExt {
    fn name(&self) -> String;
}

impl CrateExt for Crate {
    fn name(&self) -> String {
        self.index[&self.root].name.as_ref().unwrap().clone()
    }
}

trait ItemKindExt {
    fn kind(&self) -> Option<ItemKind>;
}

impl ItemKindExt for Item {
    fn kind(&self) -> Option<ItemKind> {
        match &self.inner {
            ItemEnum::Module(_) => Some(ItemKind::Module),
            ItemEnum::Struct(_) => Some(ItemKind::Struct),
            ItemEnum::Enum(_) => Some(ItemKind::Enum),
            ItemEnum::Function(_) => Some(ItemKind::Function),
            ItemEnum::Trait(_) => Some(ItemKind::Trait),
            ItemEnum::TypeAlias(_) => Some(ItemKind::Type),
            ItemEnum::Constant { .. } => Some(ItemKind::Constant),
            ItemEnum::Macro(_) => Some(ItemKind::Macro),
            _ => None,
        }
    }
}

trait ImplExt {
    fn is_inherent(&self) -> bool;
    fn inherent_impl_type_id(&self) -> Option<&ItemId>;
}

impl ImplExt for Impl {
    fn is_inherent(&self) -> bool {
        self.trait_.is_none()
            && self.blanket_impl.is_none()
            && matches!(self.for_, Type::ResolvedPath(_))
    }

    fn inherent_impl_type_id(&self) -> Option<&ItemId> {
        if self.trait_.is_some() || self.blanket_impl.is_some() {
            // not an inherent impl
            return None;
        }

        match &self.for_ {
            Type::ResolvedPath(path) => Some(&path.id),
            _ => None,
        }
    }
}

trait WithItemExt<T> {
    fn with_item(&self, v: T) -> Self;
}

impl<T: Clone> WithItemExt<T> for Vec<T> {
    fn with_item(&self, v: T) -> Self {
        let mut out = Self::with_capacity(self.len() + 1);
        out.extend_from_slice(self);
        out.push(v);
        out
    }
}
