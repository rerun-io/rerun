use std::sync::LazyLock;

use ahash::HashMapExt;
use nohash_hasher::IntMap;

use re_types::{ComponentDescriptor, ComponentName};

use crate::Chunk;

// ---

impl Chunk {
    // TODO: explain
    // TODO: this particular impl only make sense for <=0.21, right?
    /// Looks for builtin Rerun components in the chunk, and automatically
    pub fn autotagged(&self) -> Self {
        re_tracing::profile_function!();

        let mut chunk = self.clone();

        // TODO: explain the vec
        static DESCRIPTORS_PER_COMPONENT: LazyLock<
            IntMap<ComponentName, Vec<ComponentDescriptor>>,
        > = LazyLock::new(|| {
            re_tracing::profile_scope!("init reflection");

            let mut descriptors_per_component = IntMap::new();

            let Ok(reflection) = re_types::reflection::generate_reflection() else {
                // TODO: warn (err?)
                return descriptors_per_component;
            };

            // TODO: we need the actual field name...
            // TODO: we need the actual field scope...
            for (archetype_name, info) in reflection
                .archetypes
                .iter()
                // We only care about patching
                .filter(|(_name, info)| info.scope == Some("blueprint"))
            {
                for field in &info.fields {
                    let descriptor = ComponentDescriptor {
                        archetype_name: Some(*archetype_name),
                        archetype_field_name: Some(field.name.into()),
                        component_name: field.component_name,
                    };
                    descriptors_per_component
                        .entry(field.component_name)
                        .or_default()
                        .push(descriptor);
                }
            }

            for (component_name, descriptors) in &descriptors_per_component {
                if descriptors.len() > 1 {
                    eprintln!("* Ambiguity detected! `{component_name}` is used in:");
                    for desc in descriptors {
                        eprintln!("    * `{desc}`");
                    }
                }
            }

            descriptors_per_component
        });

        let per_component_name = &mut chunk.components;
        for (component_name, per_desc) in per_component_name.iter_mut() {
            if per_desc.len() != 1 {
                // If there are more than one entry, then we're in the land of UB anyway (for now).
                continue;
            }

            let Some((desc, list_array)) = std::mem::take(per_desc).into_iter().next() else {
                // This is unreachable, but I'd rather not unwrap.
                continue;
            };

            if desc.archetype_name.is_some() || desc.archetype_field_name.is_some() {
                // It's already tagged, leave it alone.
                per_desc.insert(desc, list_array);
                break;
            }

            if let Some(descriptors) = (*DESCRIPTORS_PER_COMPONENT).get(component_name) {
                for descr in descriptors {
                    per_desc.insert(descr.clone(), list_array.clone());
                }
            } else {
                per_desc.insert(desc, list_array);
            }
        }

        chunk
    }
}
