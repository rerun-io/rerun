use arrow::array::{
    Array as ArrowArray, ArrayRef as ArrowArrayRef, StringArray as ArrowStringArray,
};
use itertools::Itertools as _;

use re_arrow_util::ArrowArrayDowncastRef as _;
use re_types_core::arrow_helpers::as_array_ref;

use crate::Chunk;

impl Chunk {
    /// We look for indicator component descriptors that have an archetype name and strip it.
    ///
    /// It turns out that too narrow indicator descriptors cause porblems while querying.
    /// More information: <https://github.com/rerun-io/rerun/pull/9938#issuecomment-2888808593>
    #[inline]
    pub fn patched_weak_indicator_descriptor_023_compat(&self) -> Self {
        let mut chunk = self.clone();

        chunk.components = chunk
            .components
            .0
            .drain()
            .map(|(mut descriptor, list_array)| {
                if descriptor.component_name.is_indicator_component() {
                    descriptor.archetype_name = None;
                }
                (descriptor, list_array)
            })
            .collect();

        chunk
    }

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

        let mut components_descriptor_patched_from_to = Vec::new();
        for (descriptor, list_array) in chunk.components.iter_mut() {
            // Second, patch all descriptors that still use space-view terminology.
            let mut patched_descriptor = descriptor.clone();
            for (from, to) in PATCHES {
                if let Some(archetype_name) = descriptor.archetype_name {
                    patched_descriptor.archetype_name =
                        Some(archetype_name.as_str().replace(from, to).into());
                }
                patched_descriptor.component_name =
                    descriptor.component_name.replace(from, to).into();
            }
            if patched_descriptor != *descriptor {
                components_descriptor_patched_from_to
                    .push((descriptor.clone(), patched_descriptor));
            }

            // Finally, patch actual data that still uses space-view terminology.
            // As far as we know, this only concerns `IncludedContent` specifically.
            if descriptor.component_name == "rerun.blueprint.components.IncludedContent" {
                let arrays = list_array
                    .iter()
                    .map(|utf8_array| {
                        utf8_array.map(|array| -> ArrowArrayRef {
                            let Some(array) = array.downcast_array_ref::<ArrowStringArray>() else {
                                // Unreachable, just avoiding unwraps.
                                return array;
                            };

                            as_array_ref(
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
                                    .collect::<ArrowStringArray>(),
                            )
                        })
                    })
                    .collect_vec();
                let arrays = arrays
                    .iter()
                    .map(|a| a.as_deref() as Option<&dyn ArrowArray>)
                    .collect_vec();

                if let Some(list_array_patched) = re_arrow_util::arrays_to_list_array_opt(&arrays) {
                    *list_array = list_array_patched;
                }
            }
        }

        for (desc_from, desc_to) in components_descriptor_patched_from_to {
            if let Some(list_array) = chunk.components.remove(&desc_from) {
                chunk.components.insert(desc_to, list_array);
            }
        }

        chunk
    }
}
