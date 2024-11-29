//! Force-directed graph layouts assume edges to be straight lines. This
//! function brings edges in a canonical form and finds the ones that occupy the same space.

use crate::graph::{EdgeId, NodeId};

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
    /// A regular edge going from `source` to `target`.
    Regular,
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
        let slot = slots
            .entry(SlotId::new(e.source, e.target))
            .or_insert_with(|| Slot {
                kind: match e.source == e.target {
                    true => SlotKind::SelfEdge,
                    false => SlotKind::Regular,
                },
                edges: Vec::new(),
            });

        slot.edges.push(e);
    }

    slots
}

// TODO(grtlr): Write test cases
