use crate::arrow_ui::ArrowNode;
use arrow::array::{Array, AsArray, StructArray, UnionArray};
use std::sync::Arc;

/// Iterator over child nodes of an Arrow array.
///
/// For some kinds, this will hold a reference to the parent array and an index.
/// For others (lists), it will hold a reference to the child array.
pub enum ChildNodes<'a> {
    List(Arc<dyn Array>),
    Struct {
        parent_index: usize,
        array: &'a StructArray,
    },
    /// Map where the key is shown as the node name
    InlineKeyMap {
        keys: MaybeArc<'a>,
        values: MaybeArc<'a>,
        parent_index: usize,
    },
    /// Map where key and value are shown as separate nodes
    Map {
        keys: MaybeArc<'a>,
        values: MaybeArc<'a>,
        parent_index: usize,
    },
    Union {
        array: &'a UnionArray,
        parent_index: usize,
    },
}

impl<'a> ChildNodes<'a> {
    pub fn new(array: &'a dyn Array, index: usize) -> Option<Self> {
        let child_nodes = if let Some(struct_array) = array.as_struct_opt() {
            ChildNodes::Struct {
                parent_index: index,
                array: struct_array,
            }
        } else if let Some(list) = array.as_list_opt::<i32>() {
            let value = list.value(index);
            ChildNodes::List(value.clone())
        } else if let Some(list) = array.as_list_opt::<i64>() {
            let value = list.value(index);
            ChildNodes::List(value.clone())
        } else if let Some(list_array) = array.as_fixed_size_list_opt() {
            let value = list_array.value(index);
            ChildNodes::List(value.clone())
        } else if let Some(dict_array) = array.as_any_dictionary_opt() {
            if !dict_array.keys().data_type().is_nested() {
                ChildNodes::InlineKeyMap {
                    keys: dict_array.keys().into(),
                    values: dict_array.values().clone().into(),
                    parent_index: index,
                }
            } else {
                ChildNodes::Map {
                    keys: dict_array.keys().into(),
                    values: dict_array.values().clone().into(),
                    parent_index: index,
                }
            }
        } else if let Some(map_array) = array.as_map_opt() {
            // if !map_array.keys().data_type().is_nested() {
            //     ChildNodes::InlineKeyMap {
            //         keys: map_array.keys().clone().into(),
            //         values: map_array.values().clone().into(),
            //         parent_index: index,
            //     }
            // } else {
            //     ChildNodes::Map {
            //         keys: map_array.keys().clone().into(),
            //         values: map_array.values().clone().into(),
            //         parent_index: index,
            //     }
            // }
            let entries = map_array.entries();
            ChildNodes::Struct {
                parent_index: index,
                array: entries,
            }
        } else if let Some(union_array) = array.as_union_opt() {
            ChildNodes::Union {
                array: union_array,
                parent_index: index,
            }
        } else {
            return None;
        };

        Some(child_nodes)
    }

    pub fn len(&self) -> usize {
        match self {
            ChildNodes::List(array) => array.len(),
            ChildNodes::Struct {
                parent_index: _,
                array,
            } => array.num_columns(),
            ChildNodes::InlineKeyMap { keys, .. } => keys.as_ref().len() * 2, // TODO: Implement inline thingy
            ChildNodes::Map { keys, .. } => keys.as_ref().len() * 2,
            ChildNodes::Union {
                array: _union_array,
                parent_index: _,
            } => 1,
        }
    }

    /// Ui is needed to style the name of `InlineKeyMap` nodes
    pub fn get_child(&self, index: usize) -> crate::arrow_ui::ArrowNode<'a> {
        assert!(index < self.len(), "Index out of bounds: {index}");
        match self {
            ChildNodes::List(list) => crate::arrow_ui::ArrowNode::new(list.clone(), index),
            ChildNodes::Struct {
                parent_index: struct_index,
                array,
            } => {
                let column = array.column(index);
                let name = array.column_names()[index];
                crate::arrow_ui::ArrowNode::new(&**column, *struct_index).with_field_name(name)
            }
            ChildNodes::InlineKeyMap {
                keys,
                values,
                parent_index,
            } => {
                // TODO: Implement inline node
                // let key_node = crate::arrow_ui::ArrowNode::new(keys.clone(), *parent_index);
                // let key_job = key_node.layout_job(ui);
                // crate::arrow_ui::ArrowNode::new(values.clone(), *parent_index)
                //     .with_field_name(key_job)

                let is_key = index % 2 == 0;
                let actual_index = index / 2;

                if is_key {
                    crate::arrow_ui::ArrowNode::new(keys.clone(), actual_index)
                        .with_field_name("key")
                } else {
                    crate::arrow_ui::ArrowNode::new(values.clone(), actual_index)
                        .with_field_name("value")
                }
            }
            ChildNodes::Map {
                keys,
                values,
                parent_index,
            } => {
                let is_key = index % 2 == 0;
                let actual_index = index / 2;

                if is_key {
                    crate::arrow_ui::ArrowNode::new(keys.clone(), actual_index)
                        .with_field_name("key")
                } else {
                    crate::arrow_ui::ArrowNode::new(values.clone(), actual_index)
                        .with_field_name("value")
                }
            }
            ChildNodes::Union {
                array: union_array,
                parent_index,
            } => {
                let variant_index = union_array.type_id(*parent_index);
                let child = union_array.child(variant_index);
                let names = union_array.type_names();
                let variant_name = names
                    .get(variant_index as usize)
                    .expect("Variant index should be valid");
                ArrowNode::new(child.clone(), *parent_index).with_field_name(*variant_name)
            }
        }
    }

    pub fn iter(&self) -> impl NodeIterator<'_> {
        (0..self.len()).map(move |index| self.get_child(index))
    }
}

pub trait NodeIterator<'a>:
    Iterator<Item = crate::arrow_ui::ArrowNode<'a>> + ExactSizeIterator
{
}

impl<'a, I: Iterator<Item = crate::arrow_ui::ArrowNode<'a>> + ExactSizeIterator> NodeIterator<'a>
    for I
{
}

#[derive(Debug, Clone)]
pub enum MaybeArc<'a> {
    Array(&'a dyn Array),
    Arc(arrow::array::ArrayRef),
}

impl MaybeArc<'_> {
    pub fn as_ref(&self) -> &dyn Array {
        match self {
            MaybeArc::Array(array) => *array,
            MaybeArc::Arc(arc) => arc.as_ref(),
        }
    }
}

impl<'a> From<&'a dyn Array> for MaybeArc<'a> {
    fn from(array: &'a dyn Array) -> Self {
        MaybeArc::Array(array)
    }
}

impl From<arrow::array::ArrayRef> for MaybeArc<'_> {
    fn from(array: arrow::array::ArrayRef) -> Self {
        MaybeArc::Arc(array)
    }
}
