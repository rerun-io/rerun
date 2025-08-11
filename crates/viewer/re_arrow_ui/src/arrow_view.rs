use crate::arrow_node::ArrowNode;
use crate::arrow_ui::make_formatter;
use crate::child_nodes::{ChildNodes, MaybeArc};
use arrow::array::{Array, ArrayAccessor, ArrayRef, AsArray, MapArray, StructArray};
use arrow::datatypes::DataType;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum ArrayView<'a> {
    List(MaybeArc<'a>),
    /// A list of maps
    MapArray(&'a MapArray, bool),
    MapArrayOwned(MapArray, bool),
    /// A single map (with multiple keys and values) of a [`ArrayView::MapArray`]
    Map(StructArray, bool),
}

impl<'a> ArrayView<'a> {
    pub fn new(array: &'a dyn Array) -> Self {
        if let Some(map_array) = array.as_map_opt() {
            let inline = !map_array.keys().data_type().is_nested();
            // TODO: It'd be nicer to derive this from the data type, but apparently this needs
            // the outer data type?
            if map_array.len() > 1 {
                Self::MapArray(map_array, inline)
            } else {
                Self::Map(map_array.value(0), inline)
            }
        } else {
            Self::List(array.into())
        }
    }

    pub fn new_ref(array: Arc<dyn Array>) -> Self {
        if let Some(map_array) = array.as_map_opt() {
            let inline = !map_array.data_type().is_nested();
            if map_array.len() > 1 {
                Self::MapArrayOwned(map_array.clone(), inline)
            } else {
                Self::Map(map_array.value(0), inline)
            }
        } else {
            Self::List(array.into())
        }
    }

    pub fn as_array(&self) -> &dyn Array {
        match self {
            ArrayView::List(list) => list.as_ref(),
            ArrayView::MapArray(list, ..) => list as &dyn Array,
            ArrayView::MapArrayOwned(list, ..) => list as &dyn Array,
            ArrayView::Map(list, ..) => list as &dyn Array,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::List(array) => array.as_ref().len(),
            Self::MapArray(map, ..) => map.len(),
            Self::MapArrayOwned(map, ..) => map.len(),
            Self::Map(map, ..) => map.len(),
        }
    }

    pub fn child_nodes(&self, index: usize) -> Option<ChildNodes<'_>> {
        match self {
            Self::MapArray(map, inline) => {
                let array = map.value(index);
                Some(ChildNodes::List(ArrayView::Map(array, *inline)))
            }
            Self::MapArrayOwned(map, inline) => {
                let array = map.value(index);
                Some(ChildNodes::List(ArrayView::Map(array, *inline)))
            }
            Self::Map(struct_array, _inline) => Some(ChildNodes::Map {
                keys: struct_array.column(0).clone().into(),
                values: struct_array.column(1).clone().into(),
                parent_index: index,
            }),
            Self::List(_) => ChildNodes::new(self.as_array(), index),
        }
    }

    pub fn node(&self, index: usize) -> ArrowNode<'_> {
        if let Self::Map(struct_array, inline) = self {
            if *inline {
                let keys = struct_array.column(0);
                let values = struct_array.column(1);
                let formatter = make_formatter(keys).unwrap();
                let key = formatter(index);
                return ArrowNode::new(
                    values.clone(),
                    index,
                    ChildNodes::new(values.as_ref(), index),
                )
                .with_field_name(key);
            }
        }

        ArrowNode::new(self.as_array(), index, self.child_nodes(index))
    }
}
