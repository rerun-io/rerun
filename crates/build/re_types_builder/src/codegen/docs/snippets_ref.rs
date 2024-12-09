//! Generate the snippets reference.

#![allow(dead_code)]

use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use itertools::Itertools;

use crate::{CodeGenerator, GeneratedFiles, Object, ObjectKind, Objects, Reporter};

// ---

/// See `docs/snippets/snippets.toml` for more info
#[derive(Debug, serde::Deserialize)]
struct Config {
    snippets_ref: SnippetsRef,
}

#[derive(Debug, serde::Deserialize)]
struct SnippetsRef {
    archetypes: Archetypes,
}

// TODO: custom_sort obviously takes precedence
#[derive(Debug, serde::Deserialize)]
struct Archetypes {
    opt_out: BTreeMap<String, Vec<String>>,
    custom_sort: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
struct Snippet<'o> {
    path: PathBuf,
    name: String,

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

#[derive(Default, Debug)]
struct Snippets<'o> {
    per_archetype: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_component: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_archetype_blueprint: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_component_blueprint: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
    per_view: BTreeMap<&'o Object, Vec<Snippet<'o>>>,
}

impl<'o> Snippets<'o> {
    fn merge(&mut self, rhs: Self) {
        let Self {
            per_archetype,
            per_component,
            per_archetype_blueprint,
            per_component_blueprint,
            per_view,
        } = self;
        per_archetype.extend(rhs.per_archetype);
        per_component.extend(rhs.per_component);
        per_archetype_blueprint.extend(rhs.per_archetype_blueprint);
        per_component_blueprint.extend(rhs.per_component_blueprint);
        per_view.extend(rhs.per_view);
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

        let mut files_to_write = GeneratedFiles::default();

        let snippets_dir = re_build_tools::cargo_metadata()
            .context("failed to read cargo metadata")?
            .workspace_root
            .join("docs/snippets");

        let snippets_dir_root = snippets_dir.join("all");
        let snippets_dir_archetypes = snippets_dir_root.clone();

        let config_path = snippets_dir.join("snippets.toml");
        let config = std::fs::read_to_string(config_path)?;
        let config: Config = toml::from_str(&config)?;

        // dbg!(&config);

        let known_objects = KnownObjects::init(objects);
        // dbg!(&known_objects);

        let snippets = collect_snippets_recursively(
            &known_objects,
            &snippets_dir_archetypes,
            &config,
            &snippets_dir_root,
        )?;
        // dbg!(&snippets);

        fn snippet_row(obj: &Object, snippet: &Snippet<'_>) -> String {
            const SNIPPETS_URL: &str =
                "https://github.com/rerun-io/rerun/blob/latest/docs/snippets/all";

            let name = &obj.name;
            let name_snake = obj.snake_case_name();
            let snippet_name = &snippet.name;

            #[allow(unwrap_used)]
            let snippet_kind = snippet
                .path
                .components()
                .rev()
                .nth(1)
                .unwrap()
                .as_os_str()
                .to_string_lossy();

            let kind = match obj.kind {
                ObjectKind::Datatype => "datatypes",
                ObjectKind::Component => "components",
                ObjectKind::Archetype => "archetypes",
                ObjectKind::View => "views",
            };

            let description = snippet.description.clone().unwrap_or_default();

            let link_docs = format!("https://rerun.io/docs/reference/types/{kind}/{name_snake}");
            let rendered_name =
                if obj.kind != ObjectKind::View && obj.scope().as_deref() == Some("blueprint") {
                    // Blueprint types have no website pages yet.
                    format!("`{name}`")
                } else {
                    format!("[`{name}`]({link_docs})")
                };

            let link_py = if snippet.python {
                let link = format!("{SNIPPETS_URL}/{snippet_kind}/{snippet_name}.py");
                format!("[🐍]({link})")
            } else {
                String::new()
            };
            let link_rs = if snippet.rust {
                let link = format!("{SNIPPETS_URL}/{snippet_kind}/{snippet_name}.rs");
                format!("[🦀]({link})")
            } else {
                String::new()
            };
            let link_cpp = if snippet.cpp {
                let link = format!("{SNIPPETS_URL}/{snippet_kind}/{snippet_name}.cpp");
                format!("[🌊]({link})")
            } else {
                String::new()
            };

            format!("| {rendered_name} | `{snippet_name}` | {description} | {link_py} | {link_rs} | {link_cpp} |")
        }

        fn snippet_table(snippets: &BTreeMap<&Object, Vec<Snippet<'_>>>) -> String {
            snippets
                .iter()
                .flat_map(|(obj, snippets)| {
                    let mut snippets = snippets.clone();
                    snippets.sort_by(|a, b| {
                        if a.name.contains(&obj.snake_case_name()) {
                            // Snippets that contain the object in question in their name should
                            // bubble up to the top.
                            Ordering::Less
                        } else {
                            a.name.cmp(&b.name)
                        }
                    });
                    snippets.into_iter().map(move |snippet| (obj, snippet))
                })
                .map(|(obj, snippet)| snippet_row(obj, &snippet))
                .collect_vec()
                .join("\n")
        }

        let per_archetype_table = snippet_table(&snippets.per_archetype);
        let per_component_table = snippet_table(&snippets.per_component);
        let per_archetype_blueprint_table = snippet_table(&snippets.per_archetype_blueprint);
        let per_component_blueprint_table = snippet_table(&snippets.per_component_blueprint);
        let per_view_table = snippet_table(&snippets.per_view);

        let autogen_warning = format!(
            "<!-- DO NOT EDIT! This file was auto-generated by {} -->",
            file!().replace('\\', "/")
        );
        let out = format!(
            "
{autogen_warning}

# Snippet reference

This file acts as an index reference for all of our [snippets](./README.md).

Use it to quickly find copy-pastable snippets of code for any Rerun feature you're interested in (API, Archetypes, Components, etc).

---

*Table of contents:*
* [Features](#features)
* [Types](#types)
    * [Archetypes](#archetypes)
    * [Components](#components)
    * [Views](#views-blueprint)
    * [Archetypes (blueprint)](#archetypes-blueprint)
    * [Components (blueprint)](#components-blueprint)


## Features

| Name | Example | Description |
| ---- | ------- | ----------- |

<!-- TODO: that one is fully manual, via custom snippets.toml sections -->
<!-- TODO: descriptors -->
<!-- TODO: dataframes -->
<!-- TODO: tutorials: anyvalue, send blueprint, etc -->
<!-- TODO: and many many other things -->


## Types


### Archetypes

_All snippets, organized by the [`Archetype`](https://rerun.io/docs/reference/types/archetypes)(s) they use._

_Autogenerated, with optional opt-out and explicit sorting support._

| Archetype | Example | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_archetype_table}


### Components

_All snippets, organized by the [`Component`](https://rerun.io/docs/reference/types/components)(s) they use._

| Component | Example | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_component_table}


### Views (blueprint)

_All snippets, organized by the [`View`](https://rerun.io/docs/reference/types/views)(s) they use._

| Component | Example | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_view_table}


### Archetypes (blueprint)

_All snippets, organized by the blueprint-related [`Archetype`](https://rerun.io/docs/reference/types/archetypes)(s) they use._

| Archetype | Example | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_archetype_blueprint_table}


### Components (blueprint)

_All snippets, organized by the blueprint-related [`Component`](https://rerun.io/docs/reference/types/components)(s) they use._

| Component | Example | Description | Python | Rust | C++ |
| --------- | ------- | ----------- | ------ | ---- | --- |
{per_component_blueprint_table}
"
        );

        files_to_write.insert(self.out_dir.join("INDEX.md"), out.trim().to_owned());

        Ok(files_to_write)
    }
}

fn collect_snippets_recursively<'o>(
    known_objects: &KnownObjects<'o>,
    dir: &Utf8Path,
    config: &Config,
    snippet_root_path: &Utf8Path,
) -> anyhow::Result<Snippets<'o>> {
    let mut snippets = Snippets::default();

    #[allow(clippy::unwrap_used)] // we just use unwrap for string <-> path conversion here
    for snippet in dir.read_dir()? {
        let snippet = snippet?;
        let meta = snippet.metadata()?;
        let path = snippet.path();

        let name = path.file_stem().unwrap().to_str().unwrap().to_owned();

        // let config_key = path.strip_prefix(snippet_root_path)?.with_extension("");
        // let config_key = config_key.to_str().unwrap().replace('\\', "/");

        // let is_opted_out = config
        //     .opt_out
        //     .run
        //     .get(&config_key)
        //     .is_some_and(|languages| languages.iter().any(|v| v == "py"));
        // if is_opted_out {
        //     continue;
        // }

        if meta.is_dir() {
            snippets.merge(collect_snippets_recursively(
                known_objects,
                Utf8Path::from_path(&path).unwrap(),
                config,
                snippet_root_path,
            )?);
            continue;
        }

        if !path.extension().is_some_and(|p| p == "py") {
            continue;
        }

        let contents = std::fs::read_to_string(&path)?;
        let description = contents.lines().take(1).next().and_then(|s| {
            s.contains("\"\"\"")
                .then(|| s.replace("\"\"\"", "").trim_end_matches('.').to_owned())
        });

        let mut archetypes = BTreeSet::default();
        let mut components = BTreeSet::default();
        let mut archetypes_blueprint = BTreeSet::default();
        let mut components_blueprint = BTreeSet::default();
        let mut views = BTreeSet::default();

        for obj in &known_objects.archetypes {
            if contents.contains(&obj.name) {
                archetypes.insert(*obj);
                continue;
            }
        }
        for obj in &known_objects.components {
            if contents.contains(&obj.name) {
                components.insert(*obj);
                continue;
            }
        }
        for obj in &known_objects.archetypes_blueprint {
            if contents.contains(&obj.name) {
                archetypes_blueprint.insert(*obj);
                continue;
            }
        }
        for obj in &known_objects.components_blueprint {
            if contents.contains(&obj.name) {
                components_blueprint.insert(*obj);
                continue;
            }
        }
        for obj in &known_objects.views {
            if contents.contains(&obj.name) {
                views.insert(*obj);
                continue;
            }
        }

        let python = true;
        let rust = path.with_extension("rs").exists();
        let cpp = path.with_extension("cpp").exists();

        let snippet = Snippet {
            name,
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

        for obj in &snippet.archetypes {
            snippets
                .per_archetype
                .entry(obj)
                .or_default()
                .push(snippet.clone());
        }
        for obj in &snippet.components {
            snippets
                .per_component
                .entry(obj)
                .or_default()
                .push(snippet.clone());
        }
        for obj in &snippet.archetypes_blueprint {
            snippets
                .per_archetype_blueprint
                .entry(obj)
                .or_default()
                .push(snippet.clone());
        }
        for obj in &snippet.components_blueprint {
            snippets
                .per_component_blueprint
                .entry(obj)
                .or_default()
                .push(snippet.clone());
        }
        for obj in &snippet.views {
            snippets
                .per_view
                .entry(obj)
                .or_default()
                .push(snippet.clone());
        }
    }

    Ok(snippets)
}

#[derive(Debug)]
struct KnownObjects<'o> {
    archetypes: Vec<&'o Object>,
    components: Vec<&'o Object>,

    archetypes_blueprint: Vec<&'o Object>,
    components_blueprint: Vec<&'o Object>,

    views: Vec<&'o Object>,
}

impl<'o> KnownObjects<'o> {
    pub fn init(objects: &'o Objects) -> Self {
        let (
            mut archetypes,
            mut components,
            mut archetypes_blueprint,
            mut components_blueprint,
            mut views,
        ) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());

        for object in objects.values() {
            // skip test-only archetypes
            if object.is_testing() {
                continue;
            }

            match object.kind {
                ObjectKind::Archetype if object.scope().as_deref() == Some("blueprint") => {
                    archetypes_blueprint.push(object);
                }

                ObjectKind::Archetype => {
                    archetypes.push(object);
                }

                ObjectKind::Component if object.scope().as_deref() == Some("blueprint") => {
                    components_blueprint.push(object);
                }

                ObjectKind::Component => {
                    components.push(object);
                }

                ObjectKind::View => {
                    views.push(object);
                }

                ObjectKind::Datatype => {}
            };
        }

        archetypes.sort_by(|a, b| a.fqname.cmp(&b.fqname));
        components.sort_by(|a, b| a.fqname.cmp(&b.fqname));
        archetypes_blueprint.sort_by(|a, b| a.fqname.cmp(&b.fqname));
        components_blueprint.sort_by(|a, b| a.fqname.cmp(&b.fqname));
        views.sort_by(|a, b| a.fqname.cmp(&b.fqname));

        Self {
            archetypes,
            components,
            archetypes_blueprint,
            components_blueprint,
            views,
        }
    }
}
