//! Private module with the AST-like definitions of lenses.
//!
//! **Note**: Apart from high-level entry points (like [`Op`] and [`Lens`],
//! we should not leak these elements into the public API. This allows us to
//! evolve the definition of lenses over time, if requirements change.

use arrow::{array::ListArray, datatypes::DataType};

use re_chunk::{Chunk, ChunkComponents, ChunkId, ComponentIdentifier, EntityPath};
use re_log_types::EntityPathFilter;
use re_types::ComponentDescriptor;

use super::{Error, op};
use re_arrow_util::transform::{Flatten, Transform as _};

pub struct InputColumn {
    pub entity_path_filter: EntityPathFilter,
    pub component: ComponentIdentifier,
}

pub struct OutputColumn {
    /// The target entity path for this column.
    ///
    /// If `None`, the entity path of the output column will be set to the matched entity path of the input column.
    pub new_entity_path: Option<EntityPath>,
    pub component_descr: ComponentDescriptor,
    pub ops: Vec<Op>,
    // TODO(grtlr): It would be much nicer if static could be inferred from the output of the operations?
    pub is_static: bool,
}

type CustomFn = Box<dyn Fn(&ListArray) -> Result<ListArray, Error> + Sync + Send>;

/// Provides commonly used transformations of component columns.
///
/// Individual operations are wrapped to hide their implementation details.
pub enum Op {
    /// Extracts a specific field from a `StructArray`.
    AccessField(op::AccessField),

    /// Efficiently casts a component to a new `DataType`.
    Cast(op::Cast),

    /// Flattens a nested list array by one level.
    Flatten,

    /// A user-defined arbitrary function to convert a component column.
    Func(CustomFn),
}

impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessField(inner) => f.debug_tuple("AccessField").field(inner).finish(),
            Self::Cast(inner) => f.debug_tuple("Cast").field(inner).finish(),
            Self::Flatten => f.debug_tuple("Flatten").finish(),
            Self::Func(_) => f.debug_tuple("Func").field(&"<function>").finish(),
        }
    }
}

impl Op {
    /// Extracts a specific field from a `StructArray`.
    pub fn access_field(field_name: impl Into<String>) -> Self {
        Self::AccessField(op::AccessField {
            field_name: field_name.into(),
        })
    }

    /// Efficiently casts a component to a new `DataType`.
    pub fn cast(data_type: DataType) -> Self {
        Self::Cast(op::Cast {
            to_inner_type: data_type,
        })
    }

    /// Flattens a nested list array by one level.
    ///
    /// Takes `List<List<T>>` and flattens it to `List<T>` by concatenating all inner lists
    /// within each outer list row.
    pub fn flatten() -> Self {
        Self::Flatten
    }

    /// Ignores any input and returns a constant `ListArray`.
    ///
    /// Commonly used with [`LensBuilder::add_static_output_column_entity`].
    /// When used in non-static columns this function will _not_ guarantee the correct amount of rows.
    pub fn constant(value: ListArray) -> Self {
        Self::func(move |_| Ok(value.clone()))
    }

    /// A user-defined arbitrary function to convert a component column.
    pub fn func<F>(func: F) -> Self
    where
        F: for<'a> Fn(&'a ListArray) -> Result<ListArray, Error> + Send + Sync + 'static,
    {
        Self::Func(Box::new(func))
    }
}

impl Op {
    fn call(&self, list_array: &ListArray) -> Result<ListArray, Error> {
        match self {
            Self::Cast(op) => op.call(list_array),
            Self::AccessField(op) => op.call(list_array),
            Self::Flatten => Flatten::new().transform(list_array).map_err(Into::into),
            Self::Func(func) => func(list_array),
        }
    }
}

/// A lens that transforms component data from one form to another.
///
/// Lenses allow you to extract, transform, and restructure component data. They
/// are applied to chunks that match the specified entity path filter and contain
/// the target component.
///
/// # Assumptions
///
/// Works on component columns within a chunk. Because what goes into a chunk
/// is non-deterministic, and dependent on the batcher, no assumptions should be
/// made for values across rows.
pub struct Lens {
    input: InputColumn,
    outputs: Vec<OutputColumn>,
}

impl Lens {
    /// Returns a new [`LensBuilder`] with the given input column.
    pub fn for_input_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> LensBuilder {
        LensBuilder::for_input_column(entity_path_filter, component)
    }
}

impl Lens {
    /// Applies this lens and crates one or more chunks.
    fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        let found = chunk
            .components()
            .iter()
            .find(|(descr, _array)| descr.component == self.input.component);

        // This means we drop chunks that belong to the same entity but don't have the component.
        let Some((_component_descr, list_array)) = found else {
            return Default::default();
        };

        let mut builders = ahash::HashMap::default();
        for output in &self.outputs {
            let entity_path = output
                .new_entity_path
                .as_ref()
                .unwrap_or(chunk.entity_path());

            let components = builders
                .entry((entity_path.clone(), output.is_static))
                .or_insert_with(ChunkComponents::default);

            if components.contains_component(&output.component_descr) {
                re_log::warn_once!("Replacing duplicated component {}", output.component_descr);
            }

            let mut list_array_result = list_array.clone();
            for op in &output.ops {
                match op.call(&list_array_result) {
                    Ok(result) => {
                        list_array_result = result;
                    }
                    Err(err) => {
                        re_log::error!(
                            "Lens operation '{:?}' failed for output column '{}' on entity '{}': {err}",
                            op,
                            entity_path,
                            output.component_descr.component
                        );
                        return vec![];
                    }
                }
            }

            components.insert(output.component_descr.clone(), list_array_result);
        }

        builders
            .into_iter()
            .filter_map(|((entity_path, is_static), components)| {
                let timelines = if is_static {
                    Default::default()
                } else {
                    chunk.timelines().clone()
                };

                // TODO(grtlr): In case of static, should we use sparse rows instead?
                Chunk::from_auto_row_ids(ChunkId::new(), entity_path.clone(), timelines, components)
                    .inspect_err(|err| {
                        re_log::error_once!(
                            "Failed to build chunk at entity path '{entity_path}': {err}"
                        );
                    })
                    .ok()
            })
            .collect()
    }
}

/// Provides convenient function to create a [`Lens`].
pub struct LensBuilder(Lens);

impl LensBuilder {
    /// The column on which this [`Lens`] will operate on.
    ///
    /// For now, no substitutions will be performed on [`EntityPathFilter`].
    pub fn for_input_column(
        entity_path_filter: EntityPathFilter,
        component: impl Into<ComponentIdentifier>,
    ) -> Self {
        Self(Lens {
            input: InputColumn {
                entity_path_filter,
                component: component.into(),
            },
            outputs: vec![],
        })
    }

    /// Can be used to define one or more output columns that are derived from the
    /// component specified via [`Self::for_input_column`].
    ///
    /// The output column will live on the same entity path as the input column.
    pub fn add_output_column(
        mut self,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        let column = OutputColumn {
            new_entity_path: None,
            component_descr,
            ops: ops.into_iter().collect(),
            is_static: false,
        };
        self.0.outputs.push(column);
        self
    }

    /// Can be used to define one or more output columns that are derived from the
    /// component specified via [`Self::for_input_column`].
    ///
    /// The output column will live on the same entity path as the input column.
    pub fn add_output_column_entity(
        mut self,
        entity_path: impl Into<EntityPath>,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        let column = OutputColumn {
            new_entity_path: Some(entity_path.into()),
            component_descr,
            ops: ops.into_iter().collect(),
            is_static: false,
        };
        self.0.outputs.push(column);
        self
    }

    /// Can be used to define one or more static output columns that are derived from the
    /// component specified via [`Self::for_input_column`].
    ///
    /// The output column will live on the same entity path as the input column.
    ///
    /// In most cases, static columns should have a single row only.
    // TODO(grtlr): We don't provide a non-entity version of this method, because it is
    //              likely to change again anyway.
    pub fn add_static_output_column_entity(
        mut self,
        entity_path: impl Into<EntityPath>,
        component_descr: ComponentDescriptor,
        ops: impl IntoIterator<Item = Op>,
    ) -> Self {
        let column = OutputColumn {
            new_entity_path: Some(entity_path.into()),
            component_descr,
            ops: ops.into_iter().collect(),
            is_static: true,
        };
        self.0.outputs.push(column);
        self
    }

    /// Finalizes this builder and returns the corresponding lens.
    pub fn build(self) -> Lens {
        self.0
    }
}

#[derive(Default)]
pub struct LensRegistry {
    lenses: Vec<Lens>,
}

impl LensRegistry {
    pub fn add_lens(&mut self, lens: Lens) {
        self.lenses.push(lens);
    }

    fn relevant(&self, chunk: &Chunk) -> impl Iterator<Item = &Lens> {
        self.lenses.iter().filter(|lens| {
            lens.input
                .entity_path_filter
                .clone()
                .resolve_without_substitutions()
                .matches(chunk.entity_path())
        })
    }

    /// Applies all relevant lenses to a chunk and returns the transformed chunks.
    ///
    /// This will only transform component columns that match registered lenses.
    /// Other component columns are dropped. To retain original data, use identity
    /// lenses or multi-sink configurations.
    pub fn apply(&self, chunk: &Chunk) -> Vec<Chunk> {
        self.relevant(chunk)
            .flat_map(|transform| transform.apply(chunk))
            .collect()
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use re_chunk::{
        TimeColumn, TimelineName,
        external::arrow::{
            array::{
                Float32Builder, Float64Builder, Int32Builder, ListBuilder, StringBuilder,
                StructBuilder,
            },
            datatypes::{DataType, Field},
        },
    };
    use re_types::{ComponentDescriptor, archetypes::Scalars};

    use super::*;

    /// Creates a chunk that contains all sorts of validity, nullability, and empty lists.
    // ┌──────────────┬───────────┐
    // │ [{a:0,b:0}]  │ ["zero"]  │
    // ├──────────────┼───────────┤
    // │[{a:1,b:null}]│["one","1"]│
    // ├──────────────┼───────────┤
    // │      []      │    []     │
    // ├──────────────┼───────────┤
    // │     null     │ ["three"] │
    // ├──────────────┼───────────┤
    // │ [{a:4,b:4}]  │   null    │
    // ├──────────────┼───────────┤
    // │    [null]    │ ["five"]  │
    // ├──────────────┼───────────┤
    // │ [{a:6,b:6}]  │  [null]   │
    // └──────────────┴───────────┘
    fn nullability_chunk() -> Chunk {
        let mut struct_column_builder = ListBuilder::new(StructBuilder::new(
            [
                Arc::new(Field::new("a", DataType::Float32, true)),
                Arc::new(Field::new("b", DataType::Float64, true)),
            ],
            vec![
                Box::new(Float32Builder::new()),
                Box::new(Float64Builder::new()),
            ],
        ));
        let mut string_column_builder = ListBuilder::new(StringBuilder::new());

        // row 0
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(0.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(0.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_value("zero");
        string_column_builder.append(true);

        // row 1
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(1.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_null();
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_value("one");
        string_column_builder.values().append_value("1");
        string_column_builder.append(true);

        // row 2
        struct_column_builder.append(true); // empty list

        string_column_builder.append(true); // empty list

        // row 3
        struct_column_builder.append(false); // null

        string_column_builder.values().append_value("three");
        string_column_builder.append(true);

        // row 4
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(4.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(4.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.append(false); // null

        // row 5
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_null(); // placeholder for null struct
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_null(); // placeholder for null struct
        struct_column_builder.values().append(false); // null struct element
        struct_column_builder.append(true);

        string_column_builder.values().append_value("five");
        string_column_builder.append(true);

        // row 6
        struct_column_builder
            .values()
            .field_builder::<Float32Builder>(0)
            .unwrap()
            .append_value(6.0);
        struct_column_builder
            .values()
            .field_builder::<Float64Builder>(1)
            .unwrap()
            .append_value(6.0);
        struct_column_builder.values().append(true);
        struct_column_builder.append(true);

        string_column_builder.values().append_null();
        string_column_builder.append(true);

        let struct_column = struct_column_builder.finish();
        let string_column = string_column_builder.finish();

        let components = [
            (ComponentDescriptor::partial("structs"), struct_column),
            (ComponentDescriptor::partial("strings"), string_column),
        ]
        .into_iter();

        let time_column = TimeColumn::new_sequence("tick", [0, 1, 2, 3, 4, 5, 6]);

        Chunk::from_auto_row_ids(
            ChunkId::new(),
            "nullability".into(),
            std::iter::once((TimelineName::new("tick"), time_column)).collect(),
            components.collect(),
        )
        .unwrap()
    }

    #[test]
    fn test_destructure_cast() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "structs")
                .add_output_column_entity(
                    "nullability/a",
                    Scalars::descriptor_scalars(),
                    [Op::access_field("a"), Op::cast(DataType::Float64)],
                )
                .build();

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_cast", format!("{chunk:-240}"));
    }

    #[test]
    fn test_destructure() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let destructure =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "structs")
                .add_output_column_entity(
                    "nullability/b",
                    Scalars::descriptor_scalars(),
                    [Op::access_field("b")],
                )
                .build();

        let pipeline = LensRegistry {
            lenses: vec![destructure],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("destructure_only", format!("{chunk:-240}"));
    }

    #[test]
    fn test_inner_count() {
        let original_chunk = nullability_chunk();
        println!("{original_chunk}");

        let count_fn = |list_array: &ListArray| {
            let mut builder = ListBuilder::new(Int32Builder::new());

            for maybe_array in list_array.iter() {
                match maybe_array {
                    None => builder.append_null(),
                    Some(component_batch_array) => {
                        builder
                            .values()
                            .append_value(component_batch_array.len() as i32);
                        builder.append(true);
                    }
                }
            }

            Ok(builder.finish())
        };

        let count =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "strings")
                .add_output_column(ComponentDescriptor::partial("counts"), [Op::func(count_fn)])
                .add_output_column(
                    ComponentDescriptor::partial("original"),
                    [], // no operations
                )
                .build();

        let pipeline = LensRegistry {
            lenses: vec![count],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("inner_count", format!("{chunk:-240}"));
    }

    #[test]
    fn test_static_chunk_creation() {
        let original_chunk = nullability_chunk();

        let mut metadata_builder_a = ListBuilder::new(StringBuilder::new());
        metadata_builder_a
            .values()
            .append_value("static_metadata_a");
        metadata_builder_a.append(true);

        let mut metadata_builder_b = ListBuilder::new(StringBuilder::new());
        metadata_builder_b
            .values()
            .append_value("static_metadata_b");
        metadata_builder_b.append(true);

        let static_lens =
            Lens::for_input_column(EntityPathFilter::parse_forgiving("nullability"), "strings")
                .add_static_output_column_entity(
                    "nullability/static",
                    ComponentDescriptor::partial("static_metadata_a"),
                    [Op::constant(metadata_builder_a.finish())],
                )
                .add_static_output_column_entity(
                    "nullability/static",
                    ComponentDescriptor::partial("static_metadata_b"),
                    [Op::constant(metadata_builder_b.finish())],
                )
                .build();

        let pipeline = LensRegistry {
            lenses: vec![static_lens],
        };

        let res = pipeline.apply(&original_chunk);
        assert_eq!(res.len(), 1);

        let chunk = &res[0];
        insta::assert_snapshot!("single_static", format!("{chunk:-240}"));
    }
}
