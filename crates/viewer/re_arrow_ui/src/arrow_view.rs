use crate::arrow_node::ArrowNode;
use crate::child_nodes::{ChildNodes, MaybeArc};
use arrow::array::{Array, ArrayAccessor, ArrayRef, AsArray, MapArray, StructArray};
use arrow::datatypes::DataType;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ArrowView<'a> {
    List(MaybeArc<'a>),
    /// A list of maps
    MapArray(&'a MapArray),
    MapArrayOwned(MapArray),
    /// A single map (with multiple keys and values) of a [`Self::MapArray`]
    Map(StructArray),
}

impl<'a> ArrowView<'a> {
    pub fn new(array: &'a dyn Array) -> Self {
        if let Some(map_array) = array.as_map_opt() {
            if map_array.len() > 1 {
                Self::MapArray(map_array)
            } else {
                Self::Map(map_array.value(0))
            }
        } else {
            Self::List(array.into())
        }
    }

    pub fn new_ref(array: Arc<dyn Array>) -> Self {
        if let Some(map_array) = array.as_map_opt() {
            if map_array.len() > 1 {
                Self::MapArrayOwned(map_array.clone())
            } else {
                Self::Map(map_array.value(0))
            }
        } else {
            Self::List(array.into())
        }
    }

    pub fn as_array(&self) -> &dyn Array {
        match self {
            ArrowView::List(list) => list.as_ref(),
            ArrowView::MapArray(list) => list as &dyn Array,
            ArrowView::MapArrayOwned(list) => list as &dyn Array,
            ArrowView::Map(list) => list as &dyn Array,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::List(array) => array.as_ref().len(),
            Self::MapArray(map) => map.len(),
            Self::MapArrayOwned(map) => map.len(),
            Self::Map(map) => map.len(),
        }
    }

    pub fn child_nodes(&self, index: usize) -> Option<ChildNodes<'_>> {
        match self {
            Self::MapArray(map) => {
                let array = map.value(index);
                Some(ChildNodes::List(ArrowView::Map(array)))
            }
            Self::MapArrayOwned(map) => {
                let array = map.value(index);
                Some(ChildNodes::List(ArrowView::Map(array)))
            }
            Self::Map(struct_array) => Some(ChildNodes::Map {
                keys: struct_array.column(0).clone().into(),
                values: struct_array.column(1).clone().into(),
                parent_index: index,
            }),
            Self::List(_) => ChildNodes::new(self.as_array(), index),
        }
    }

    pub fn node(&self, index: usize) -> ArrowNode<'_> {
        ArrowNode::new(self.as_array(), index, self.child_nodes(index))
    }
}
