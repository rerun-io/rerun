#![expect(clippy::unwrap_used)] // build tool, so okay here

use std::collections::HashSet;
use std::fmt::Display;
use std::fs::read_to_string;
use std::process::Command;

use camino::Utf8PathBuf;
use itertools::Itertools as _;
use roxmltree::{Children, Descendants, Document, Node};

use super::{Context, DocumentData, DocumentKind};
use crate::build_search_index::util::{CommandExt as _, ProgressBarExt as _};

macro_rules! document {
    ($dom:ident, $path:expr) => {
        let dom = read_to_string($path)?;
        let $dom = roxmltree::Document::parse(&dom)?;
    };
}

pub fn ingest(ctx: &Context) -> anyhow::Result<()> {
    let progress = ctx.progress_bar("cpp");

    progress.set("doxygen", ctx.is_tty());

    Command::new("doxygen")
        .with_arg("docs/Doxyfile")
        .with_cwd(ctx.workspace_root().join("rerun_cpp"))
        .output()?;

    let base_path = ctx.workspace_root().join("rerun_cpp/docs/xml");
    let mut visitor = Visitor {
        ctx,
        base_path,
        visited: HashSet::new(),
    };
    visitor.visit_root()?;

    ctx.finish_progress_bar(progress);

    Ok(())
}

struct Visitor<'a> {
    ctx: &'a Context,
    base_path: Utf8PathBuf,

    /// Set of visited `refid`
    visited: HashSet<String>,
}

impl Visitor<'_> {
    fn push(&mut self, id: &str, name: String, description: String, uri: impl Display) {
        if self.visited.contains(id) {
            return;
        }
        self.visited.insert(id.to_owned());

        self.ctx.push(DocumentData {
            kind: DocumentKind::Cpp,
            title: name,
            hidden_tags: vec!["c++".into(), "cpp".into()],
            tags: vec![],
            content: description,
            url: format!("https://ref.rerun.io/docs/cpp/stable/{uri}"),
        });
    }

    fn visit_root(&mut self) -> anyhow::Result<()> {
        const ROOT_NAMESPACE_REFID: &str = "namespacererun";
        document!(
            root,
            self.base_path.join(format!("{ROOT_NAMESPACE_REFID}.xml"))
        );
        self.visit_namespace_document(&root)
    }

    fn visit_children(&mut self, id: &str, children: Children<'_, '_>) -> anyhow::Result<()> {
        for node in children {
            match node.tag_name().name() {
                "innerclass" => {
                    let refid = get_attr(node, "refid")?;
                    if refid.contains("_3") {
                        // skip specializations, e.g. `AsComponents<archetypes::AnnotationContext>
                        continue;
                    }

                    let prot = get_attr(node, "prot")?;
                    if prot == "private" {
                        continue; // `\private`
                    }

                    document!(doc, self.base_path.join(format!("{refid}.xml")));
                    self.visit_innerclass_document(&doc)?;
                }
                "innernamespace" => {
                    let refid = get_attr(node, "refid")?;

                    document!(doc, self.base_path.join(format!("{refid}.xml")));
                    self.visit_namespace_document(&doc)?;
                }
                "sectiondef" => {
                    for node in node.children() {
                        if node.tag_name().name() == "memberdef" {
                            self.visit_memberdef(id, node)?;
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn visit_namespace_document(&mut self, doc: &Document<'_>) -> anyhow::Result<()> {
        let root = get_first_by_tag_name(doc, "compounddef")?;
        let id = get_attr(root, "id")?;
        self.visit_children(id, root.children())?;

        Ok(())
    }

    fn visit_innerclass_document(&mut self, doc: &Document<'_>) -> anyhow::Result<()> {
        let root = get_first_by_tag_name(doc, "compounddef")?;
        let id = get_attr(root, "id")?;
        let name = get_first_by_tag_name(&root, "compoundname")?
            .text()
            .unwrap();
        let description = parse_description(root)?;

        self.push(id, name.to_owned(), description, format_args!("{id}.html"));

        self.visit_children(id, root.children())?;

        Ok(())
    }

    fn visit_memberdef(&mut self, parent_id: &str, node: Node<'_, '_>) -> anyhow::Result<()> {
        let id = get_attr(node, "id")?;
        if id.contains("_3") {
            // skip specializations, e.g. `AsComponents<archetypes::AnnotationContext>
            return Ok(());
        }

        let kind = get_attr(node, "kind")?;
        if !matches!(kind, "typedef" | "function" | "variable") {
            return Ok(());
        }

        let prot = get_attr(node, "prot")?;
        if prot == "private" {
            return Ok(()); // `\private`
        }

        let name = get_first_by_tag_name(&node, "qualifiedname")
            .or_else(|_| get_first_by_tag_name(&node, "name"))?
            .text()
            .unwrap();
        let description = parse_description(node)?;

        if let Some(stripped_id) = id.strip_prefix(parent_id) {
            self.push(
                id,
                name.to_owned(),
                description,
                format_args!("{parent_id}.html#{}", &stripped_id["_1".len()..]),
            );
        }

        Ok(())
    }
}

fn parse_description(node: Node<'_, '_>) -> anyhow::Result<String> {
    let content = |tag: &str| -> anyhow::Result<String> {
        // retrieve all text node descendants of `tag`
        let text_nodes = node
            .children()
            .find(|n| n.tag_name().name() == tag)
            .ok_or_else(|| {
                anyhow::anyhow!("invalid XML: failed to find tag {:?}", "briefdescription")
            })?
            .descendants()
            .filter(|n| n.is_text())
            .map(|n| n.text().unwrap());

        Ok(text_nodes
            .flat_map(|text| text.split_whitespace())
            .filter(|t| !t.is_empty())
            .join(" "))
    };

    let brief = content("briefdescription")?;
    let detailed = content("detaileddescription")?;

    Ok(match (brief.is_empty(), detailed.is_empty()) {
        (true, true) => String::new(),
        (false, true) => brief,
        (true, false) => detailed,
        (false, false) => format!("{brief}{detailed}"),
    })
}

trait HasDescendants<'a, 'input> {
    fn descendants(&'a self) -> Descendants<'a, 'input>;
}

impl<'a, 'input, T> HasDescendants<'a, 'input> for &'_ T
where
    T: HasDescendants<'a, 'input>,
{
    fn descendants(&'a self) -> Descendants<'a, 'input> {
        T::descendants(self)
    }
}

impl<'a, 'input> HasDescendants<'a, 'input> for Document<'input> {
    fn descendants(&'a self) -> Descendants<'a, 'input> {
        self.descendants()
    }
}

impl<'a, 'input> HasDescendants<'a, 'input> for Node<'a, 'input> {
    fn descendants(&'a self) -> Descendants<'a, 'input> {
        self.descendants()
    }
}

fn get_first_by_tag_name<'a, 'input, El: HasDescendants<'a, 'input>>(
    el: &'a El,
    tag: &str,
) -> anyhow::Result<Node<'a, 'input>> {
    el.descendants()
        .find(|n| n.tag_name().name() == tag)
        .ok_or_else(|| anyhow::anyhow!("invalid XML: failed to find tag {tag:?}"))
}

fn get_attr<'a, 'input: 'a>(node: Node<'a, 'input>, attr: &str) -> anyhow::Result<&'a str> {
    node.attribute(attr)
        .ok_or_else(|| anyhow::anyhow!("invalid XML: missing attribute {attr:?} on node {node:?}"))
}
