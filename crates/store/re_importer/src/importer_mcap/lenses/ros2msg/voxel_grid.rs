use std::sync::Arc;

use arrow::array::builder::{FixedSizeListBuilder, Int32Builder, ListBuilder, UInt32Builder};
use arrow::array::{Array as _, ArrayRef, Float32Builder, ListArray, StructArray, UInt32Array};
use arrow::datatypes::{DataType, Field};
use re_lenses::{CastTo, Lens, LensBuilderError, op};
use re_lenses_core::Selector;
use re_lenses_core::combinators::Error;
use re_log_types::TimeType;
use re_sdk_types::Loggable as _;
use re_sdk_types::archetypes::{CoordinateFrame, VoxelGridMap};
use re_sdk_types::components::Opacity;

use crate::importer_mcap::lenses::helpers::get_field_as;

use super::ROS2_TIMESTAMP;

const MARKED_COLOR: u32 = 0xFF0000FF;
const FREE_COLOR: u32 = 0x00FF00FF;

// Use a default opacity, this looks nicer than solid colors and can be adjusted via blueprint if desired.
const DEFAULT_VOXEL_OPACITY: f32 = 0.5;

/// Creates a lens for `nav2_msgs/msg/VoxelGrid` messages.
///
/// Marked voxels are colored red, free voxels are colored green,
/// and unknown voxels are omitted in the output [`VoxelGridMap`].
pub fn voxel_grid(time_type: TimeType) -> Result<Lens, LensBuilderError> {
    let flatten = Selector::parse(".[]")?;

    Lens::derive("nav2_msgs.msg.VoxelGrid:message")
        .to_timeline(
            ROS2_TIMESTAMP,
            time_type,
            Selector::parse(".header.stamp")?.pipe(op::timespec_to_nanos()),
        )
        .to_component(
            CoordinateFrame::descriptor_frame(),
            Selector::parse(".header.frame_id")?,
        )
        .to_component(
            VoxelGridMap::descriptor_voxel_indices(),
            Selector::parse(".")?
                .pipe(extract_voxel_indices)
                .pipe(flatten.clone()),
        )
        .to_component_with_cast(
            VoxelGridMap::descriptor_voxel_size(),
            Selector::parse(".resolutions | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .to_component(
            VoxelGridMap::descriptor_colors(),
            Selector::parse(".")?
                .pipe(extract_voxel_colors)
                .pipe(flatten.clone()),
        )
        // Store also a float representation of the voxel values.
        // This can be used for queries or as an alternative coloring source in blueprints
        // (change `colors` to view default and select a colormap).
        .to_component(
            VoxelGridMap::descriptor_values(),
            Selector::parse(".")?
                .pipe(extract_voxel_values)
                .pipe(flatten),
        )
        .to_component(
            VoxelGridMap::descriptor_opacity(),
            Selector::parse(".")?.pipe(default_voxel_opacity),
        )
        .to_component_with_cast(
            VoxelGridMap::descriptor_translation(),
            Selector::parse(".origin | pack(.x!, .y!, .z!)")?,
            CastTo::Auto,
        )
        .build()
}

/// The three possible states of a voxel.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nav2VoxelState {
    Free,
    Unknown,
    Marked,
}

impl Nav2VoxelState {
    /// Decodes the two-bit Nav2 state at z: `00` free, `01` unknown, `11` marked.
    fn from_packed_column(column: u32, z: usize) -> Self {
        let z_mask = ((1_u32 << 16) | 1) << z;
        match (column & z_mask).count_ones() {
            0 => Self::Free,
            1 => Self::Unknown,
            _ => Self::Marked,
        }
    }

    fn color(self) -> Result<u32, Error> {
        match self {
            Self::Free => Ok(FREE_COLOR),
            Self::Unknown => {
                // We don't include unknown voxels in the sparse output, this should be unreachable.
                Err(Error::Other(
                    "nav2 voxel grid emitted unknown voxel state".to_owned(),
                ))
            }
            Self::Marked => Ok(MARKED_COLOR),
        }
    }

    fn value(self) -> Result<f32, Error> {
        match self {
            Self::Free => Ok(0.),
            Self::Unknown => {
                // We don't include unknown voxels in the sparse output, this should be unreachable.
                Err(Error::Other(
                    "nav2 voxel grid emitted unknown voxel state".to_owned(),
                ))
            }
            Self::Marked => Ok(1.),
        }
    }
}

/// Extracts sparse voxel indices for [`VoxelGridMap`] from densely packed Nav2 voxel columns.
fn extract_voxel_indices(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let grid_input = nav2_voxel_grid_input(source, "extract_nav2_voxel_indices")?;
    // Pre-allocate only the outer list slots: the row count is exact, while the inner
    // sparse voxel count would require another decode pass or an oversized dense bound.
    let mut builder = ListBuilder::with_capacity(voxel_indices_builder(), grid_input.len());

    grid_input.for_each_row(|grid| {
        let Some(grid) = grid else {
            builder.append_null();
            return Ok(());
        };

        let indices_builder = builder.values();
        grid.for_each_known_voxel(|voxel| {
            indices_builder.values().append_value(voxel.x_index);
            indices_builder.values().append_value(voxel.y_index);
            indices_builder.values().append_value(voxel.z_index);
            indices_builder.append(true);
            Ok(())
        })?;
        builder.append(true);
        Ok(())
    })?;

    Ok(Some(Arc::new(builder.finish()) as ArrayRef))
}

/// Extracts per-voxel colors for free and marked states.
fn extract_voxel_colors(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let grid_input = nav2_voxel_grid_input(source, "extract_nav2_voxel_colors")?;
    let mut builder = ListBuilder::with_capacity(UInt32Builder::new(), grid_input.len());

    grid_input.for_each_row(|grid| {
        let Some(grid) = grid else {
            builder.append_null();
            return Ok(());
        };

        // Use the same traversal order as `extract_nav2_voxel_indices` so the
        // nth color corresponds to the nth emitted voxel index.
        grid.for_each_known_voxel(|voxel| {
            builder.values().append_value(voxel.state.color()?);
            Ok(())
        })?;
        builder.append(true);
        Ok(())
    })?;

    Ok(Some(Arc::new(builder.finish()) as ArrayRef))
}

/// Extracts per-voxel float representations of free and marked states.
fn extract_voxel_values(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let grid_input = nav2_voxel_grid_input(source, "extract_nav2_voxel_values")?;
    let mut builder = ListBuilder::with_capacity(Float32Builder::new(), grid_input.len());

    grid_input.for_each_row(|grid| {
        let Some(grid) = grid else {
            builder.append_null();
            return Ok(());
        };

        // Use the same traversal order as `extract_nav2_voxel_indices` so the
        // nth value corresponds to the nth emitted voxel index.
        grid.for_each_known_voxel(|voxel| {
            builder.values().append_value(voxel.state.value()?);
            Ok(())
        })?;
        builder.append(true);
        Ok(())
    })?;

    Ok(Some(Arc::new(builder.finish()) as ArrayRef))
}

fn voxel_indices_builder() -> FixedSizeListBuilder<Int32Builder> {
    FixedSizeListBuilder::new(Int32Builder::new(), 3).with_field(Field::new(
        "item",
        DataType::Int32,
        false,
    ))
}

fn default_voxel_opacity(source: &ArrayRef) -> Result<Option<ArrayRef>, Error> {
    let opacity = Opacity::from(DEFAULT_VOXEL_OPACITY);
    Opacity::to_arrow_opt(std::iter::repeat_n(Some(opacity), source.len()))
        .map(Some)
        .map_err(|err| Error::Other(err.to_string()))
}

/// Typed Arrow array handles for `nav2_msgs/msg/VoxelGrid`.
///
/// Note: cloned Arrow arrays share the underlying buffers; no voxel data is copied here.
struct Nav2VoxelGridInput<'a> {
    source: &'a StructArray,
    data_array: ListArray,
    data_values: UInt32Array,
    size_x_array: UInt32Array,
    size_y_array: UInt32Array,
    size_z_array: UInt32Array,
}

impl<'a> Nav2VoxelGridInput<'a> {
    fn len(&self) -> usize {
        self.source.len()
    }

    /// Iterates over every source row, preserving null rows as `None`.
    fn for_each_row(
        &'a self,
        mut visit: impl FnMut(Option<Nav2VoxelGridRow<'a>>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for row in 0..self.source.len() {
            visit(self.row(row)?)?;
        }
        Ok(())
    }

    /// Validates dimensions and borrows the packed `(x, y)` column data for one message row.
    fn row(&'a self, row: usize) -> Result<Option<Nav2VoxelGridRow<'a>>, Error> {
        if self.source.is_null(row)
            || self.data_array.is_null(row)
            || self.size_x_array.is_null(row)
            || self.size_y_array.is_null(row)
            || self.size_z_array.is_null(row)
        {
            return Ok(None);
        }

        let size_x = self.size_x_array.value(row) as usize;
        let size_y = self.size_y_array.value(row) as usize;
        // Nav2 stores one packed column per `(x, y)` cell; z is encoded inside that u32.
        let size_z = (self.size_z_array.value(row) as usize).min(16);
        let expected_len = size_x
            .checked_mul(size_y)
            .ok_or_else(|| Error::Other("nav2 voxel grid dimensions overflow".to_owned()))?;
        let start = self.data_array.value_offsets()[row] as usize;
        let end = self.data_array.value_offsets()[row + 1] as usize;

        if end - start != expected_len {
            return Err(Error::Other(format!(
                "nav2 voxel grid expected {} columns from {}x{} grid, got {}",
                expected_len,
                size_x,
                size_y,
                end - start
            )));
        }

        Ok(Some(Nav2VoxelGridRow {
            data: &self.data_values.values()[start..end],
            size_x,
            size_y,
            size_z,
        }))
    }
}

fn nav2_voxel_grid_input<'a>(
    source: &'a ArrayRef,
    context: &'static str,
) -> Result<Nav2VoxelGridInput<'a>, Error> {
    let source = source
        .as_any()
        .downcast_ref::<StructArray>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "StructArray".to_owned(),
            actual: source.data_type().clone(),
            context: format!("{context} input"),
        })?;

    let data_array = get_field_as::<ListArray>(source, "data")?;
    let size_x_array = get_field_as::<UInt32Array>(source, "size_x")?;
    let size_y_array = get_field_as::<UInt32Array>(source, "size_y")?;
    let size_z_array = get_field_as::<UInt32Array>(source, "size_z")?;
    let data_values = data_array
        .values()
        .as_any()
        .downcast_ref::<UInt32Array>()
        .ok_or_else(|| Error::TypeMismatch {
            expected: "UInt32Array".to_owned(),
            actual: data_array.values().data_type().clone(),
            context: format!("{context} data values"),
        })?
        .clone();

    Ok(Nav2VoxelGridInput {
        source,
        data_array,
        data_values,
        size_x_array,
        size_y_array,
        size_z_array,
    })
}

/// Borrowed view into one Nav2 voxel-grid message row.
///
/// `data` points into [`Nav2VoxelGridInput::data_values`].
struct Nav2VoxelGridRow<'a> {
    data: &'a [u32],
    size_x: usize,
    size_y: usize,
    size_z: usize,
}

impl Nav2VoxelGridRow<'_> {
    /// Voxel extraction helper that iterates over known voxels and calls the `visit` closure on each.
    ///
    /// The order is deterministic so the output columns of callers for indices, colors, and values stay aligned.
    fn for_each_known_voxel(
        &self,
        mut visit: impl FnMut(Nav2Voxel) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for y in 0..self.size_y {
            let y_index = nav2_voxel_grid_index_to_i32("y", y)?;
            for x in 0..self.size_x {
                let x_index = nav2_voxel_grid_index_to_i32("x", x)?;
                let column = self.data[y * self.size_x + x];
                for z in 0..self.size_z {
                    let state = Nav2VoxelState::from_packed_column(column, z);
                    if state == Nav2VoxelState::Unknown {
                        // Rerun's `VoxelGridMap` is sparse, so we can omit unknown cells.
                        continue;
                    }

                    visit(Nav2Voxel {
                        x_index,
                        y_index,
                        z_index: nav2_voxel_grid_index_to_i32("z", z)?,
                        state,
                    })?;
                }
            }
        }

        Ok(())
    }
}

/// One emitted sparse voxel with its decoded state.
struct Nav2Voxel {
    x_index: i32,
    y_index: i32,
    z_index: i32,
    state: Nav2VoxelState,
}

fn nav2_voxel_grid_index_to_i32(axis: &str, index: usize) -> Result<i32, Error> {
    i32::try_from(index).map_err(|err| {
        Error::Other(format!(
            "nav2 voxel grid {axis} index exceeds i32 range: {err}"
        ))
    })
}
