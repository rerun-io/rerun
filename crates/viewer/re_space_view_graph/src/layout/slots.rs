//! Force-directed graph layouts assume edges to be straight lines. This
//! function brings edges in a canonical form and finds the ones that occupy the same space.

use crate::graph::NodeId;

use super::request::EdgeTemplate;

/// Uniquely identifies an [`EdgeSlot`] by ordering the [`NodeIds`](NodeId).
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotKind {
    /// An edge slot going from `source` to `target`. Source and target represent the canonical order of the slot, as specified by [`SlotId`]
    Regular {
        source: NodeId,
        target: NodeId,
    },
    /// An edge where `source == target`.
    SelfEdge,
}

pub struct Slot<'a> {
    pub kind: SlotKind,
    pub edges: Vec<&'a EdgeTemplate>,
}

pub fn slotted_edges<'a>(
    edges: impl Iterator<Item = &'a EdgeTemplate>,
) -> ahash::HashMap<SlotId, Slot<'a>> {
    let mut slots: ahash::HashMap<SlotId, Slot<'a>> = ahash::HashMap::default();

    for e in edges {
        let id = SlotId::new(e.source, e.target);
        let slot = slots
            .entry(id)
            .or_insert_with_key(|id| Slot {
                kind: match e.source == e.target {
                    true => SlotKind::SelfEdge,
                    false => SlotKind::Regular {
                        source: id.0,
                        target: id.1,
                    },
                },
                edges: Vec::new(),
            });

        slot.edges.push(e);
    }

    slots
}

// TODO(grtlr): Write test cases
