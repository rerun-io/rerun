//! Generate the snippets reference.

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};

use crate::{CodeGenerator, GeneratedFiles, Object, ObjectKind, Objects, Reporter};

// ---

/// Everything we know about a snippet, including which objects (archetypes, components, etc) it references.
#[allow(dead_code)]
#[derive(Debug, Clone)]
struct Snippet<'o> {
    path: PathBuf,
    name: String,
    name_qualified: String,

    python: bool,
    rust: bool,
    cpp: bool,

    description: Option<String>,
    contents: String,

    archetypes: BTreeSet<&'o Object>,
    components: BTreeSet<&'o Object>,
    archetypes_blueprint: BTreeSet<&'o Object>,
    components_blueprint: BTreeSet<&'o Object>,
    views: BTreeSet<&'o Object>,
}

/// Maps objects (archetypes, components, etc) back to snippets.
#[derive(Default, Debug)]
struct Snippets<'o> {
    per_archetype: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_component: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_archetype_blueprint: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_component_blueprint: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_view: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
}

impl<'o> Snippets<'o> {
    fn merge_extend(&mut self, rhs: Self) {
        let Self {
            per_archetype,
            per_component,
            per_archetype_blueprint,
            per_component_blueprint,
            per_view,
        } = self;

        let merge_extend = |a: &mut BTreeMap<&'o Object, Vec<Snippet<'o>>>, b| {
            for (obj, snippets) in b {
                a.entry(obj).or_default().extend(snippets);
            }
        };

        merge_extend(per_archetype, rhs.per_archetype);
        merge_extend(per_component, rhs.per_component);
        merge_extend(per_view, rhs.per_view);
        merge_extend(per_archetype_blueprint, rhs.per_archetype_blueprint);
        merge_extend(per_component_blueprint, rhs.per_component_blueprint);
    }
}

// ---

pub struct SnippetsRefCodeGenerator {
    out_dir: Utf8PathBuf,
}

impl SnippetsRefCodeGenerator {
    pub fn new(out_dir: impl Into<Utf8PathBuf>) -> Self {
        Self {
            out_dir: out_dir.into(),
        }
    }
}

impl CodeGenerator for SnippetsRefCodeGenerator {
    fn generate(
        &mut self,
        reporter: &Reporter,
        objects: &Objects,
        _arrow_registry: &crate::ArrowRegistry,
    ) -> GeneratedFiles {
        match self.generate_fallible(objects) {
            Ok(files) => files,
            Err(err) => {
                reporter.error_any(err);
                Default::default()
            }
        }
    }
}

impl SnippetsRefCodeGenerator {
    fn generate_fallible(&self, objects: &Objects) -> anyhow::Result<GeneratedFiles> {
        re_tracing::profile_function!();

        const SNIPPETS_URL: &str =
            "https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all";

        let mut files_to_write = GeneratedFiles::default();

        let snippets_dir = re_build_tools::cargo_metadata()
            .context("failed to read cargo metadata")?
            .workspace_root
            .join("docs/snippets");

        let snippets_dir_root = snippets_dir.join("all");
        let snippets_dir_archetypes = snippets_dir_root.clone();

        let known_objects = KnownObjects::init(objects);

        let snippets = collect_snippets_recursively(
            &known_objects,
            &snippets_dir_archetypes,
            &snippets_dir_root,
        )?;

        /// Generates a single row for one of the autogenerated tables.
        fn snippet_row(obj: &Object, snippet: &Snippet<'_>) -> anyhow::Result<String> {
            let obj_name = &obj.name;
            let obj_name_snake = obj.snake_case_name();
            let obj_kind = match obj.kind {
                ObjectKind::Datatype => "datatypes",
                ObjectKind::Component => "components",
                ObjectKind::Archetype => "archetypes",
                ObjectKind::View => "views",
            };

            let link_docs =
                format!("https://rerun.io/docs/reference/types/{obj_kind}/{obj_name_snake}");
            let link_docs = make_speculative_if_needed(obj_name, &link_docs)?;
            let obj_name_rendered =
                if obj.kind != ObjectKind::View && obj.scope().as_deref() == Some("blueprint") {
                    // Blueprint types have no website pages yet.
                    format!("`{obj_name}`")
                } else {
                    format!("[`{obj_name}`]({link_docs})")
                };

            let snippet_name_qualified = &snippet.name_qualified;
            let snippet_descr = snippet.description.clone().unwrap_or_default();

            let link_py = if snippet.python {
                let link = format!("{SNIPPETS_URL}/{snippet_name_qualified}.py");
                let link = make_speculative_if_needed(obj_name, &link)?;
                let link = make_speculative_if_needed(snippet_name_qualified, &link)?;
                format!("[🐍]({link})")
            } else {
                String::new()
            };
            let link_rs = if snippet.rust {
                let link = format!("{SNIPPETS_URL}/{snippet_name_qualified}.rs");
                let link = make_speculative_if_needed(obj_name, &link)?;
                let link = make_speculative_if_needed(snippet_name_qualified, &link)?;
                format!("[🦀]({link})")
            } else {
                String::new()
            };
            let link_cpp = if snippet.cpp {
                let link = format!("{SNIPPETS_URL}/{snippet_name_qualified}.cpp");
                let link = make_speculative_if_needed(obj_name, &link)?;
                let link = make_speculative_if_needed(snippet_name_qualified, &link)?;
                format!("[🌊]({link})")
            } else {
                String::new()
            };

            let row = format!("| **{obj_name_rendered}** | `{snippet_name_qualified}` | {snippet_descr} | {link_py} | {link_rs} | {link_cpp} |");

            Ok(row)
        }

        let snippets_table = |snippets: &BTreeMap<&Object, Vec<Snippet<'_>>>| {
            let table = snippets
                .iter()
                .flat_map(|(obj, snippets)| {
                    let mut snippets = snippets.clone();
                    // NOTE: Gotta sort twice to make sure it stays stable after the second one.
                    snippets.sort_by(|a, b| a.name_qualified.cmp(&b.name_qualified));
                    snippets.sort_by(|a, b| {
                        if a.name_qualified.contains(&obj.snake_case_name()) {
                            // Snippets that contain the object in question in their name should
                            // bubble up to the top.
                            Ordering::Less
                        } else {
                            a.name_qualified.cmp(&b.name_qualified)
                        }
                    });
                    snippets.into_iter().map(move |snippet| (obj, snippet))
                })
                .map(|(obj, snippet)| snippet_row(obj, &snippet))
                .collect::<Result<Vec<_>, _>>()?
                .join("\n");

            Ok::<_, anyhow::Error>(table)
        };

        let per_archetype_table = snippets_table(&snippets.per_archetype)?;
        let per_component_table = snippets_table(&snippets.per_component)?;
        let per_archetype_blueprint_table = snippets_table(&snippets.per_archetype_blueprint)?;
        let per_component_blueprint_table = snippets_table(&snippets.per_component_blueprint)?;
        let per_view_table = snippets_table(&snippets.per_view)?;

        let autogen_warning = format!(
            "<!-- DO NOT EDIT! This file was auto-generated by {} -->",
            file!().replace('\\', "/")
        );
        let out = format!(
            "
{autogen_warning}

# Snippet index reference

This file acts as an index reference for all of our [snippets](./README.md).

Use it to quickly find copy-pastable snippets of code for any Rerun feature you're interested in (API, Archetypes, Components, etc).

---

*Table of contents:*
* [Types](#types)
    * [Archetypes](#archetypes)
    * [Components](#components)
    * [Views](#views-blueprint)
    * [Archetypes (blueprint)](#archetypes-blueprint)
    * [Components (blueprint)](#components-blueprint)


## Types


### Archetypes

_All snippets, organized by the [`Archetype`](https://rerun.io/docs/reference/types/archetypes)(s) they use._

| Archetype | Snippet | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_archetype_table}


### Components

_All snippets, organized by the [`Component`](https://rerun.io/docs/reference/types/components)(s) they use._

| Component | Snippet | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_component_table}


### Views (blueprint)

_All snippets, organized by the [`View`](https://rerun.io/docs/reference/types/views)(s) they use._

| Component | Snippet | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_view_table}


### Archetypes (blueprint)

_All snippets, organized by the blueprint-related [`Archetype`](https://rerun.io/docs/reference/types/archetypes)(s) they use._

| Archetype | Snippet | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_archetype_blueprint_table}


### Components (blueprint)

_All snippets, organized by the blueprint-related [`Component`](https://rerun.io/docs/reference/types/components)(s) they use._

| Component | Snippet | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_component_blueprint_table}
"
        );

        #[allow(clippy::string_add)]
        files_to_write.insert(self.out_dir.join("INDEX.md"), out.trim().to_owned() + "\n");

        Ok(files_to_write)
    }
}

fn collect_snippets_recursively<'o>(
    known_objects: &KnownObjects<'o>,
    dir: &Utf8Path,
    snippet_root_path: &Utf8Path,
) -> anyhow::Result<Snippets<'o>> {
    let mut snippets = Snippets::default();

    #[allow(clippy::unwrap_used)] // we just use unwrap for string <-> path conversion here
    for snippet in dir.read_dir()? {
        let snippet = snippet?;
        let meta = snippet.metadata()?;
        let path = snippet.path();

        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();
        let name_qualified = path.strip_prefix(snippet_root_path)?.with_extension("");
        let name_qualified = name_qualified.to_str().unwrap().replace('\\', "/");

        if meta.is_dir() {
            snippets.merge_extend(collect_snippets_recursively(
                known_objects,
                Utf8Path::from_path(&path).unwrap(),
                snippet_root_path,
            )?);
            continue;
        }

        // We only track the Python one. We'll derive the other two from there, if they exist at all.
        if !path.extension().is_some_and(|p| p == "py") {
            continue;
        }

        let contents = std::fs::read_to_string(&path)?;
        let description = contents.lines().take(1).next().and_then(|s| {
            s.contains("\"\"\"")
                .then(|| s.replace("\"\"\"", "").trim_end_matches('.').to_owned())
        });

        // All archetypes, components, etc that this snippet refers to.
        let mut archetypes = BTreeSet::default();
        let mut components = BTreeSet::default();
        let mut archetypes_blueprint = BTreeSet::default();
        let mut components_blueprint = BTreeSet::default();
        let mut views = BTreeSet::default();

        // Fill the sets by grepping into the snippet's contents.
        for (objs, set) in [
            (&known_objects.archetypes, &mut archetypes),
            (&known_objects.components, &mut components),
            (&known_objects.views, &mut views),
            (
                &known_objects.archetypes_blueprint,
                &mut archetypes_blueprint,
            ),
            (
                &known_objects.components_blueprint,
                &mut components_blueprint,
            ),
        ] {
            for obj in objs {
                if contents.contains(&obj.name) {
                    set.insert(*obj);
                    continue;
                }
            }
        }

        let python = true;
        let rust = path.with_extension("rs").exists();
        let cpp = path.with_extension("cpp").exists();

        let snippet = Snippet {
            name,
            name_qualified,
            path,

            contents,
            description,

            python,
            rust,
            cpp,

            archetypes,
            components,
            archetypes_blueprint,
            components_blueprint,
            views,
        };

        // Fill the reverse indices.
        for (objs, index) in [
            (&snippet.archetypes, &mut snippets.per_archetype),
            (&snippet.components, &mut snippets.per_component),
            (&snippet.views, &mut snippets.per_view),
            (
                &snippet.archetypes_blueprint,
                &mut snippets.per_archetype_blueprint,
            ),
            (
                &snippet.components_blueprint,
                &mut snippets.per_component_blueprint,
            ),
        ] {
            for obj in objs {
                index.entry(obj).or_default().push(snippet.clone());
            }
        }
    }

    Ok(snippets)
}

/// Neatly organized [`Object`]s (archetypes, components, etc).
#[derive(Debug)]
struct KnownObjects<'o> {
    archetypes: BTreeSet<&'o Object>,
    components: BTreeSet<&'o Object>,

    archetypes_blueprint: BTreeSet<&'o Object>,
    components_blueprint: BTreeSet<&'o Object>,

    views: BTreeSet<&'o Object>,
}

impl<'o> KnownObjects<'o> {
    fn init(objects: &'o Objects) -> Self {
        let (
            mut archetypes,
            mut components,
            mut archetypes_blueprint,
            mut components_blueprint,
            mut views,
        ) = (
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeSet::new(),
        );

        for object in objects.values() {
            // skip test-only archetypes
            if object.is_testing() {
                continue;
            }

            match object.kind {
                ObjectKind::Archetype if object.scope().as_deref() == Some("blueprint") => {
                    archetypes_blueprint.insert(object);
                }

                ObjectKind::Archetype => {
                    archetypes.insert(object);
                }

                ObjectKind::Component if object.scope().as_deref() == Some("blueprint") => {
                    components_blueprint.insert(object);
                }

                ObjectKind::Component => {
                    components.insert(object);
                }

                ObjectKind::View => {
                    views.insert(object);
                }

                ObjectKind::Datatype => {}
            };
        }

        Self {
            archetypes,
            components,
            archetypes_blueprint,
            components_blueprint,
            views,
        }
    }
}

// ---

/// Returns `true` if the given name has not been released yet.
fn is_speculative(any_name: &str) -> anyhow::Result<bool> {
    let is_pre_0_21_release = {
        // Reminder of what those look like:
        // env!("CARGO_PKG_VERSION") = "0.21.0-alpha.1+dev"
        // env!("CARGO_PKG_VERSION_MAJOR") = "0"
        // env!("CARGO_PKG_VERSION_MINOR") = "21"
        // env!("CARGO_PKG_VERSION_PATCH") = "0"
        // env!("CARGO_PKG_VERSION_PRE") = "alpha.1"

        let minor: u32 = env!("CARGO_PKG_VERSION_MINOR")
            .parse()
            .context("couldn't parse minor crate version")?;
        let pre = env!("CARGO_PKG_VERSION_PRE");

        minor < 21 || !pre.is_empty()
    };

    const RELEASED_IN_0_21: &[&str] = &[
        // archetypes & components
        "GraphEdge",
        "GraphEdges",
        "GraphNode",
        "GraphNodes",
        "GraphView",
        "Plane3D",
        // snippets
        "concepts/explicit_recording",
        "descriptors/descr_builtin_archetype",
        "descriptors/descr_builtin_component",
        "descriptors/descr_custom_archetype",
        "descriptors/descr_custom_component",
        "howto/any_values_send_columns",
        "views/graph",
    ];

    let is_speculative = is_pre_0_21_release && RELEASED_IN_0_21.contains(&any_name);

    Ok(is_speculative)
}

/// Appends `?speculative-link` to the given link if the associated object has not been released yet.
fn make_speculative_if_needed(name: &str, link: &str) -> anyhow::Result<String> {
    if is_speculative(name)? {
        return Ok(format!("{link}?speculative-link"));
    }
    Ok(link.to_owned())
}
