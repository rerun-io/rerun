use arrow2::array::{Array, Utf8Array};

use itertools::Itertools;
use nohash_hasher::IntMap;

use crate::Chunk;

impl Chunk {
    /// A temporary migration kernel for blueprint data.
    ///
    /// Deals with all the space-view terminology breaking changes (`SpaceView`->`View`, `space_view`->`view`, etc).
    #[inline]
    pub fn patched_for_blueprint_021_compat(&self) -> Self {
        let mut chunk = self.clone();

        const PATCHES: &[(&str, &str)] = &[("SpaceView", "View"), ("space_view", "view")];

        // First, patch any entity path that still use space-view terminology.
        // We expect this to only ever be called for blueprint data, so these entity paths
        // are all builtin ones -- we're not overriding any user data.
        let mut entity_path = chunk.entity_path.to_string();
        for (from, to) in PATCHES {
            entity_path = entity_path.replace(from, to);
        }
        chunk.entity_path = entity_path.into();

        let mut components_patched = IntMap::default();
        for (component_name, per_desc) in chunk.components.iter_mut() {
            if PATCHES
                .iter()
                .any(|(from, _to)| component_name.contains(from))
            {
                // Second, patch all descriptors that still use space-view terminology.
                for (mut descriptor, list_array) in std::mem::take(per_desc) {
                    for (from, to) in PATCHES {
                        if let Some(archetype_name) = descriptor.archetype_name.as_mut() {
                            *archetype_name = archetype_name.replace(from, to).into();
                        }
                        descriptor.component_name =
                            descriptor.component_name.replace(from, to).into();
                    }
                    components_patched.insert(descriptor, list_array.clone());
                }
            }

            // Finally, patch actual data that still uses space-view terminology.
            // As far as we know, this only concerns `IncludedContent` specifically.
            if component_name == "rerun.blueprint.components.IncludedContent" {
                for list_array in per_desc.values_mut() {
                    let arrays = list_array
                        .iter()
                        .map(|utf8_array| {
                            utf8_array.map(|array| {
                                let Some(array) = array.as_any().downcast_ref::<Utf8Array<i32>>()
                                else {
                                    // Unreachable, just avoiding unwraps.
                                    return array;
                                };

                                array
                                    .iter()
                                    .map(|s| {
                                        s.map(|s| {
                                            let mut s = s.to_owned();
                                            for (from, to) in PATCHES {
                                                s = s.replace(from, to);
                                            }
                                            s
                                        })
                                    })
                                    .collect::<Utf8Array<i32>>()
                                    .to_boxed()
                            })
                        })
                        .collect_vec();
                    let arrays = arrays
                        .iter()
                        .map(|a| a.as_deref() as Option<&dyn Array>)
                        .collect_vec();

                    if let Some(list_array_patched) =
                        crate::arrow2_util::arrays_to_list_array_opt(&arrays)
                    {
                        *list_array = list_array_patched;
                    }
                }
            }
        }

        for (desc, list_array) in components_patched {
            chunk.components.insert_descriptor_arrow2(desc, list_array);
        }

        chunk
    }
}
