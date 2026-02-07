//! Force-directed graph layouts assume edges to be straight lines. A [`Slot`]
//! represents the space that a single edge, _or multiple_ edges can occupy between two nodes.
//!
//! We achieve this by bringing edges into a canonical form via [`SlotId`], which
//! we can then use to find duplicates.

use super::request::EdgeTemplate;
use crate::graph::NodeId;

/// Uniquely identifies a [`Slot`] by ordering the [`NodeIds`](NodeId) that make up an edge.
#[derive(Clone, Copy, Hash, PartialEq, Eq)]
pub struct SlotId(NodeId, NodeId);

impl SlotId {
    pub fn new(source: NodeId, target: NodeId) -> Self {
        if source < target {
            Self(source, target)
        } else {
            Self(target, source)
        }
    }
}

/// There are different types of [`Slots`](Slot) that are laid out differently.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// An edge slot going from `source` to `target`. Source and target represent the canonical order of the slot, as specified by [`SlotId`]
    Regular { source: NodeId, target: NodeId },

    /// An edge where `source == target`.
    SelfEdge { node: NodeId },
}

pub struct Slot<'a> {
    pub kind: SlotKind,
    pub edges: Vec<&'a EdgeTemplate>,
}

/// Converts a list of edges into their slotted form.
pub fn slotted_edges<'a>(
    edges: impl Iterator<Item = &'a EdgeTemplate>,
) -> ahash::HashMap<SlotId, Slot<'a>> {
    let mut slots: ahash::HashMap<SlotId, Slot<'a>> = ahash::HashMap::default();

    for e in edges {
        let id = SlotId::new(e.source, e.target);
        let slot = slots.entry(id).or_insert_with_key(|id| Slot {
            kind: if e.source == e.target {
                SlotKind::SelfEdge { node: e.source }
            } else {
                SlotKind::Regular {
                    source: id.0,
                    target: id.1,
                }
            },
            edges: Vec::new(),
        });

        slot.edges.push(e);
    }

    slots
}
