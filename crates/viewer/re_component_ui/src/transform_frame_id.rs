use std::collections::{BTreeMap, HashMap, HashSet};

use re_sdk_types::components::TransformFrameId;
use re_ui::syntax_highlighting::SyntaxHighlightedBuilder;
use re_ui::text_edit::autocomplete_text_edit;
use re_viewer_context::external::re_tf::transform_cache_snapshot;
use re_viewer_context::{
    MaybeMutRef, MissingChunkReporter, StoreViewContext, TransformDatabaseStoreCache, UiLayout,
};

/// Shows a potentially editable `frame_id`.
/// If the `frame_id` is being edited, a list of matching frame names is shown as suggestions.
pub fn edit_or_view_transform_frame_id(
    ctx: &StoreViewContext<'_>,
    ui: &mut egui::Ui,
    frame_id: &mut MaybeMutRef<'_, TransformFrameId>,
) -> egui::Response {
    match frame_id {
        MaybeMutRef::Ref(frame_id) => UiLayout::List.data_label(
            ui,
            SyntaxHighlightedBuilder::new().with_string_value(frame_id.as_str()),
        ),
        MaybeMutRef::MutRef(frame_id) => {
            let suggestions = transform_frame_suggestions(ctx, frame_id.as_str());

            let mut tmp_string = frame_id.as_str().to_owned();

            let input_error_text = if suggestions.contains_value(&tmp_string) {
                None
            } else {
                Some(format!(
                    "Choose a frame name or an implicit frame (\"{}/…\")",
                    TransformFrameId::ENTITY_HIERARCHY_PREFIX
                ))
            };

            let response = autocomplete_text_edit(
                ui,
                &mut tmp_string,
                suggestions.indented_strings(),
                None::<&str>,
                input_error_text,
            );
            if response.changed() {
                **frame_id = TransformFrameId::new(&tmp_string);
            }
            response
        }
    }
}

/// Retrieves the frame suggestions from a latest-at snapshot of the transform cache.
///
/// The snapshot is filtered based on the current input: by default only named (TF-style) frames,
/// otherwise if `tf#/…` was typed we only request implicit (entity-path-derived) frames.
fn transform_frame_suggestions(ctx: &StoreViewContext<'_>, current_text: &str) -> FrameSuggestions {
    let Some(store_ctx) = ctx.active_store_context else {
        return FrameSuggestions::default();
    };

    let implicit = current_text.starts_with(TransformFrameId::ENTITY_HIERARCHY_PREFIX);
    let filter = transform_cache_snapshot::SnapshotFilter {
        frames: if implicit {
            transform_cache_snapshot::FrameFilter::EntityPath
        } else {
            transform_cache_snapshot::FrameFilter::Named
        },
        ..Default::default()
    };

    let missing_chunk_reporter = MissingChunkReporter::default();
    let snapshot = store_ctx
        .caches
        .memoizer(|cache: &mut TransformDatabaseStoreCache| {
            cache.latest_at_transform_cache_snapshot(
                store_ctx.recording,
                &missing_chunk_reporter,
                &store_ctx.time_ctrl.current_query(),
                filter,
            )
        });

    FrameSuggestions::from_snapshot(&snapshot, implicit)
}

/// Frame names to offer as autocomplete suggestions.
///
/// The stored strings use leading whitespace to convey the transform hierarchy as indentation;
/// [`autocomplete_text_edit`] treats that indentation as display-only.
#[derive(Default)]
struct FrameSuggestions {
    indented: Vec<String>,
}

impl FrameSuggestions {
    fn from_snapshot(snapshot: &transform_cache_snapshot::Snapshot, implicit: bool) -> Self {
        // `tf#/…` frames already encode their hierarchy in the label, so we show a flat sorted list.
        if implicit {
            let mut indented = snapshot
                .frames
                .iter()
                .map(|frame| frame.label.to_string())
                .collect::<Vec<_>>();
            indented.sort();
            return Self { indented };
        }

        // Named frames: indent each suggestion by its depth in the transform hierarchy.

        // Collect the frames as nodes, remembering each frame's index by its id.
        let mut labels = Vec::new();
        let mut index_by_id = HashMap::new();
        for frame in &snapshot.frames {
            index_by_id.insert(frame.id, labels.len());
            labels.push((frame.id, frame.label.to_string()));
        }

        // Build a parent -> children adjacency map, keeping only edges between present frames.
        let mut children: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        let mut has_parent = vec![false; labels.len()];
        for edge in &snapshot.edges {
            if let (Some(&child), Some(&parent)) =
                (index_by_id.get(&edge.child), index_by_id.get(&edge.parent))
            {
                children.entry(parent).or_default().push(child);
                has_parent[child] = true;
            }
        }

        // Sort siblings alphabetically so the listing is stable regardless of edge/frame order.
        let sort_by_label = |indices: &mut Vec<usize>| {
            indices.sort_by(|a, b| labels[*a].1.cmp(&labels[*b].1));
        };
        for siblings in children.values_mut() {
            sort_by_label(siblings);
        }

        // Frames without a parent in the snapshot (or whose parent was filtered out) are roots.
        let mut roots = (0..labels.len()).filter(|i| !has_parent[*i]).collect();
        sort_by_label(&mut roots);

        // Walk the forest depth-first, prefixing two spaces per level. `visited` guards against cycles.
        let mut indented = Vec::with_capacity(labels.len());
        let mut visited = HashSet::new();
        let mut stack = roots
            .iter()
            .rev()
            .map(|&root| (root, 0usize))
            .collect::<Vec<_>>();
        while let Some((index, depth)) = stack.pop() {
            if !visited.insert(index) {
                continue;
            }
            indented.push(format!("{}{}", "  ".repeat(depth), labels[index].1));
            if let Some(siblings) = children.get(&index) {
                for &child in siblings.iter().rev() {
                    stack.push((child, depth + 1));
                }
            }
        }

        Self { indented }
    }

    /// The suggestions in display order.
    ///
    /// Named frames are ordered hierarchically and indented with leading whitespace;
    /// implicit `tf#/…` frames are a flat sorted list (their label already shows the path).
    fn indented_strings(&self) -> &[String] {
        &self.indented
    }

    fn contains_value(&self, value: &str) -> bool {
        self.indented
            .iter()
            .any(|suggestion| suggestion.trim_start() == value)
    }
}
