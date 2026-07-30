use re_log::{ResultExt as _, debug_assert_eq};

use crate::allocator::DataTextureSource;
use crate::draw_phases::PickingLayerObjectId;
use crate::renderer::gpu_data::{
    GaussianPositionScaleX, GaussianRotation, GaussianScaleYZ, GaussianShCoefficient,
};
use crate::renderer::{
    GaussianSplatBatchFlags, GaussianSplatBatchInfo, GaussianSplatDrawData,
    GaussianSplatDrawDataError, SH_TEXELS_PER_GAUSSIAN,
};
use crate::{
    CpuWriteGpuReadError, Label, OutlineMaskPreference, PickingLayerInstanceId, RenderContext,
    Rgba32Unmul,
};

/// Builder for gaussian splats, making it easy to create [`crate::renderer::GaussianSplatDrawData`].
pub struct GaussianSplatBuilder<'ctx> {
    pub(crate) ctx: &'ctx RenderContext,

    // All buffers must stay equal length.
    pub(crate) position_scale_x_buffer: DataTextureSource<'ctx, GaussianPositionScaleX>,
    pub(crate) rotation_buffer: DataTextureSource<'ctx, GaussianRotation>,
    pub(crate) scale_yz_buffer: DataTextureSource<'ctx, GaussianScaleYZ>,

    /// [`SH_TEXELS_PER_GAUSSIAN`] texels per gaussian, indexed globally by gaussian index.
    ///
    /// The shader only reads this for batches carrying
    /// [`GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS`], so the slots of gaussians without
    /// spherical harmonics are never sampled -- they only have to exist to keep the indexing
    /// aligned. Writing them is therefore deferred (see [`Self::sh_padding_gaussians`]): a cloud
    /// with no spherical harmonics at all never touches this buffer.
    pub(crate) sh_buffer: DataTextureSource<'ctx, GaussianShCoefficient>,

    /// Gaussians whose (unread) SH slots haven't been written yet.
    ///
    /// Flushed as zeros right before the next write of real coefficients, so that those land at
    /// the global index the shader expects. Left unflushed -- and thus never uploaded -- if no
    /// further coefficients arrive.
    pub(crate) sh_padding_gaussians: usize,

    /// Unmultiplied sRGB RGBA. NOT premultiplied (unlike `Color32`): premultiplying in
    /// gamma space would make the sRGB texture decode apply the gamma curve to `alpha * color`
    /// instead of the color, badly darkening the accumulation of many low-opacity gaussians.
    pub(crate) color_buffer: DataTextureSource<'ctx, Rgba32Unmul>,
    pub(crate) picking_instance_ids_buffer: DataTextureSource<'ctx, PickingLayerInstanceId>,

    pub(crate) batches: Vec<GaussianSplatBatchInfo>,
}

impl<'ctx> GaussianSplatBuilder<'ctx> {
    pub fn new(ctx: &'ctx RenderContext) -> Self {
        Self {
            ctx,
            position_scale_x_buffer: DataTextureSource::new(ctx),
            rotation_buffer: DataTextureSource::new(ctx),
            scale_yz_buffer: DataTextureSource::new(ctx),
            sh_buffer: DataTextureSource::new(ctx),
            sh_padding_gaussians: 0,
            color_buffer: DataTextureSource::new(ctx),
            picking_instance_ids_buffer: DataTextureSource::new(ctx),
            batches: Vec::with_capacity(16),
        }
    }

    /// Returns the number of gaussians that can be added without reallocation.
    /// This may be smaller than the requested number if the data texture limit is reached.
    pub fn reserve(
        &mut self,
        expected_number_of_additional_gaussians: usize,
    ) -> Result<usize, CpuWriteGpuReadError> {
        re_tracing::profile_function_if!(100_000 < expected_number_of_additional_gaussians);

        let Self {
            ctx: _,
            position_scale_x_buffer,
            rotation_buffer,
            scale_yz_buffer,
            sh_buffer: _, // Reserved lazily, only once actual coefficients show up.
            sh_padding_gaussians: _, // Ditto.
            color_buffer,
            picking_instance_ids_buffer,
            batches: _,
        } = self;

        // The maximum number is independent of datatype, so the same value applies to all buffers.
        position_scale_x_buffer.reserve(expected_number_of_additional_gaussians)?;
        rotation_buffer.reserve(expected_number_of_additional_gaussians)?;
        scale_yz_buffer.reserve(expected_number_of_additional_gaussians)?;
        color_buffer.reserve(expected_number_of_additional_gaussians)?;
        picking_instance_ids_buffer.reserve(expected_number_of_additional_gaussians)
    }

    /// Start of a new batch.
    #[inline]
    pub fn batch(&mut self, label: impl Into<Label>) -> GaussianSplatBatchBuilder<'_, 'ctx> {
        self.batches.push(GaussianSplatBatchInfo {
            label: label.into(),
            ..GaussianSplatBatchInfo::default()
        });

        GaussianSplatBatchBuilder(self)
    }

    /// Finalizes the builder and returns a draw data with all the gaussians added so far.
    pub fn into_draw_data(self) -> Result<GaussianSplatDrawData, GaussianSplatDrawDataError> {
        GaussianSplatDrawData::new(self)
    }
}

/// Writes out the SH slots of gaussians that were added without coefficients.
///
/// They are zero (and never sampled), but must exist so that subsequent real coefficients land at
/// the global index the shader computes from the gaussian index.
fn flush_sh_padding(builder: &mut GaussianSplatBuilder<'_>) {
    let num_gaussians = std::mem::take(&mut builder.sh_padding_gaussians);
    if num_gaussians == 0 {
        return;
    }

    re_tracing::profile_function!();
    builder
        .sh_buffer
        .add_n(
            GaussianShCoefficient::default(),
            num_gaussians * SH_TEXELS_PER_GAUSSIAN,
        )
        .ok_or_log_error();
}

pub struct GaussianSplatBatchBuilder<'a, 'ctx>(&'a mut GaussianSplatBuilder<'ctx>);

impl Drop for GaussianSplatBatchBuilder<'_, '_> {
    fn drop(&mut self) {
        // Remove batch again if it wasn't actually used.
        if self.0.batches.last().unwrap().gaussian_count == 0 {
            self.0.batches.pop();
        }
    }
}

impl GaussianSplatBatchBuilder<'_, '_> {
    #[inline]
    fn batch_mut(&mut self) -> &mut GaussianSplatBatchInfo {
        self.0
            .batches
            .last_mut()
            .expect("batch should have been added on GaussianSplatBatchBuilder creation")
    }

    /// Sets the `world_from_obj` matrix for the *entire* batch.
    #[inline]
    pub fn world_from_obj(mut self, world_from_obj: glam::Affine3A) -> Self {
        self.batch_mut().world_from_obj = world_from_obj;
        self
    }

    /// Provides the object-space bounds of the gaussian centers for the batch.
    ///
    /// The center of these bounds is used as the batch's draw-order sort key.
    /// If not set, the renderer computes the bounds from the gaussian centers instead.
    #[inline]
    pub fn object_space_bounding_box(
        mut self,
        object_space_bounding_box: macaw::BoundingBox,
    ) -> Self {
        self.batch_mut().object_space_bounding_box = object_space_bounding_box;
        self
    }

    /// Sets an outline mask for every gaussian in the batch.
    #[inline]
    pub fn outline_mask_ids(mut self, outline_mask_ids: OutlineMaskPreference) -> Self {
        self.batch_mut().overall_outline_mask_ids = outline_mask_ids;
        self
    }

    /// Sets the picking object id for the current batch.
    #[inline]
    pub fn picking_object_id(mut self, picking_object_id: PickingLayerObjectId) -> Self {
        self.batch_mut().picking_object_id = picking_object_id;
        self
    }

    /// Caller-owned cache holding the back-to-front ordering, persisted across frames so the
    /// renderer can seed this frame's sort from the previous one — much faster than sorting from
    /// scratch.
    ///
    /// The cache must be unique among concurrently-drawn batches.
    /// It keeps independent ordering per view when draw data is shared across views.
    /// The caller owns its lifetime and invalidation.
    #[inline]
    pub fn sort_order(mut self, sort_order_cache: crate::SortOrderCache) -> Self {
        self.batch_mut().sort_order_cache = Some(sort_order_cache);
        self
    }

    /// Pushes additional outline mask ids for a specific range of gaussians.
    /// The range is relative to this batch.
    ///
    /// Prefer the `outline_mask_ids` setting for the entire batch whenever possible!
    #[inline]
    pub fn push_additional_outline_mask_ids_for_range(
        mut self,
        range: std::ops::Range<u32>,
        ids: OutlineMaskPreference,
    ) -> Self {
        self.batch_mut()
            .additional_outline_mask_ids_vertex_ranges
            .push((range, ids));
        self
    }

    /// Adds several gaussians.
    ///
    /// All `centers` are added.
    /// The other slices are clamped to edge (their last value is repeated);
    /// if empty they fall back to: unit scale, identity rotation, white, default picking id,
    /// and no view-dependent color.
    ///
    /// `scales` are the standard deviations of the gaussians along their (rotated)
    /// principal axes, in object units.
    ///
    /// `colors` are unmultiplied sRGB RGBA, with the gaussian's peak opacity as alpha.
    ///
    /// `sh_coefficients` are optional spherical harmonics coefficients for view-dependent color:
    /// 15 per gaussian (degrees 1 through 3, coefficient-major), zero-padded for lower degrees.
    /// The degree-0 term is the gaussian's color.
    #[inline]
    pub fn add_gaussians(
        mut self,
        centers: &[glam::Vec3],
        scales: &[glam::Vec3],
        rotations: &[glam::Quat],
        colors: &[Rgba32Unmul],
        sh_coefficients: &[[GaussianShCoefficient; 15]],
        picking_ids: &[PickingLayerInstanceId],
    ) -> Self {
        re_tracing::profile_function!();

        debug_assert_eq!(
            self.0.position_scale_x_buffer.len(),
            self.0.color_buffer.len()
        );
        debug_assert_eq!(
            self.0.position_scale_x_buffer.len(),
            self.0.rotation_buffer.len()
        );
        // The shader indexes the SH texture globally, so written slots plus the deferred
        // padding must account for every gaussian so far.
        debug_assert_eq!(
            self.0.position_scale_x_buffer.len(),
            self.0.sh_buffer.len() / SH_TEXELS_PER_GAUSSIAN + self.0.sh_padding_gaussians
        );

        let num_gaussians = centers.len();

        // A batch that already carries coefficients has to keep supplying them for every one of
        // its gaussians, since the shader reads the SH texture for the whole batch.
        let writes_sh = !sh_coefficients.is_empty()
            || self
                .batch_mut()
                .flags
                .contains(GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS);

        // Do a reserve ahead of time, to check whether we're hitting the data texture limit.
        let Some(num_available) = self
            .0
            .position_scale_x_buffer
            .reserve(num_gaussians)
            .ok_or_log_error()
        else {
            return self;
        };
        // All buffers share the same element limit, but the SH buffer stores
        // `SH_TEXELS_PER_GAUSSIAN` elements per gaussian, so it runs out that many times sooner.
        let num_available = if writes_sh {
            let pending = self.0.sh_padding_gaussians;
            let Some(num_available_sh) = self
                .0
                .sh_buffer
                .reserve((pending + num_gaussians) * SH_TEXELS_PER_GAUSSIAN)
                .ok_or_log_error()
            else {
                return self;
            };
            num_available.min((num_available_sh / SH_TEXELS_PER_GAUSSIAN).saturating_sub(pending))
        } else {
            num_available
        };

        let num_gaussians = if num_gaussians > num_available {
            re_log::error_once!(
                "Reached maximum number of gaussians of {}. Ignoring all excess gaussians.",
                self.0.position_scale_x_buffer.len() + num_available
            );
            num_available
        } else {
            num_gaussians
        };

        if num_gaussians == 0 {
            return self;
        }

        let centers = &centers[..num_gaussians];
        let scales = &scales[..num_gaussians.min(scales.len())];
        let rotations = &rotations[..num_gaussians.min(rotations.len())];
        let colors = &colors[..num_gaussians.min(colors.len())];
        let sh_coefficients = &sh_coefficients[..num_gaussians.min(sh_coefficients.len())];
        let picking_ids = &picking_ids[..num_gaussians.min(picking_ids.len())];

        self.batch_mut().gaussian_count += num_gaussians as u32;

        // Retain object-space centers so the batch can be sorted back-to-front every frame.
        {
            re_tracing::profile_scope!("sort_positions");
            let sort_positions = self.batch_mut().sort_positions.get_or_insert_with(Vec::new);
            sort_positions.extend_from_slice(centers);
        }

        {
            re_tracing::profile_scope!("PosScaleX");

            let position_scale_x: Vec<_> = std::iter::zip(centers.iter().copied(), scales)
                .map(|(pos, scale)| GaussianPositionScaleX {
                    pos,
                    scale_x: scale.x,
                })
                .collect();

            self.0
                .position_scale_x_buffer
                .extend_from_slice_clamped(
                    &position_scale_x,
                    GaussianPositionScaleX {
                        pos: glam::Vec3::ZERO,
                        scale_x: 1.0,
                    },
                    num_gaussians,
                )
                .ok_or_log_error();
        }

        {
            re_tracing::profile_scope!("GaussianScaleYZ");
            let scale_yz: Vec<_> = scales
                .iter()
                .map(|scale| GaussianScaleYZ {
                    scale_y: scale.y,
                    scale_z: scale.z,
                })
                .collect();
            self.0
                .scale_yz_buffer
                .extend_from_slice_clamped(
                    &scale_yz,
                    GaussianScaleYZ {
                        scale_y: 1.0,
                        scale_z: 1.0,
                    },
                    num_gaussians,
                )
                .ok_or_log_error();
        }

        {
            re_tracing::profile_scope!("rotations");
            let rotations: &[GaussianRotation] = bytemuck::cast_slice(rotations);
            self.0
                .rotation_buffer
                .extend_from_slice_clamped(
                    rotations,
                    GaussianRotation {
                        quat_xyzw: glam::Quat::IDENTITY.to_array(),
                    },
                    num_gaussians,
                )
                .ok_or_log_error();
        }

        {
            if sh_coefficients.is_empty() {
                // Defer the (never-sampled) padding instead of uploading it now -- see
                // `GaussianSplatBuilder::sh_padding_gaussians`.
                self.0.sh_padding_gaussians += num_gaussians;
            } else {
                re_tracing::profile_scope!("sh_coefficients");
                self.batch_mut().flags |= GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS;

                // Real coefficients have to land at their global gaussian index, so any deferred
                // padding has to be materialized first.
                flush_sh_padding(self.0);

                // One texel per coefficient, so the caller's layout is already the texture's.
                self.0
                    .sh_buffer
                    .extend_from_slice(bytemuck::cast_slice(sh_coefficients))
                    .ok_or_log_error();

                // Clamp to edge: repeat the last gaussian's coefficients for any shortfall.
                if let Some(last) = sh_coefficients.last() {
                    let last: &[GaussianShCoefficient] =
                        bytemuck::cast_slice(std::slice::from_ref(last));
                    for _ in sh_coefficients.len()..num_gaussians {
                        self.0.sh_buffer.extend_from_slice(last).ok_or_log_error();
                    }
                }
            }

            // The shader samples the SH texture for every gaussian of a flagged batch, so such a
            // batch may not leave any of its own gaussians deferred.
            if self
                .batch_mut()
                .flags
                .contains(GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS)
            {
                flush_sh_padding(self.0);
            }
        }

        {
            re_tracing::profile_scope!("colors");
            self.0
                .color_buffer
                .extend_from_slice_clamped(colors, Rgba32Unmul::WHITE, num_gaussians)
                .ok_or_log_error();
        }

        {
            re_tracing::profile_scope!("picking_ids");
            self.0
                .picking_instance_ids_buffer
                .extend_from_slice_clamped(
                    picking_ids,
                    PickingLayerInstanceId::default(),
                    num_gaussians,
                )
                .ok_or_log_error();
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh_gaussians(builder: &GaussianSplatBuilder<'_>) -> usize {
        assert_eq!(builder.sh_buffer.len() % SH_TEXELS_PER_GAUSSIAN, 0);
        builder.sh_buffer.len() / SH_TEXELS_PER_GAUSSIAN
    }

    fn centers(n: usize) -> Vec<glam::Vec3> {
        (0..n).map(|i| glam::Vec3::splat(i as f32)).collect()
    }

    fn coefficients(n: usize) -> Vec<[GaussianShCoefficient; 15]> {
        vec![[GaussianShCoefficient::from_rgb([half::f16::ONE; 3]); 15]; n]
    }

    /// Gaussians without spherical harmonics must not upload anything to the SH texture:
    /// their slots are never sampled, so writing them is pure waste.
    #[test]
    fn without_spherical_harmonics_nothing_is_uploaded() {
        let ctx = RenderContext::new_test();
        let mut builder = GaussianSplatBuilder::new(&ctx);
        builder.reserve(8).expect("reserve");

        builder
            .batch("no_sh")
            .add_gaussians(&centers(8), &[], &[], &[], &[], &[]);

        assert_eq!(sh_gaussians(&builder), 0);
        assert_eq!(builder.sh_padding_gaussians, 8);
        assert!(
            !builder.batches[0]
                .flags
                .contains(GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS)
        );
    }

    /// A batch with coefficients has to cover every one of its gaussians, since the shader
    /// samples the SH texture for the whole batch.
    #[test]
    fn batch_with_spherical_harmonics_is_fully_covered() {
        let ctx = RenderContext::new_test();
        let mut builder = GaussianSplatBuilder::new(&ctx);

        {
            let mut batch = builder.batch("sh");
            batch = batch.add_gaussians(&centers(4), &[], &[], &[], &coefficients(4), &[]);
            // A follow-up add without coefficients may not leave holes behind.
            let _ = batch.add_gaussians(&centers(3), &[], &[], &[], &[], &[]);
        }

        assert_eq!(sh_gaussians(&builder), 7);
        assert_eq!(builder.sh_padding_gaussians, 0);
        assert!(
            builder.batches[0]
                .flags
                .contains(GaussianSplatBatchFlags::FLAG_HAS_SH_COEFFICIENTS)
        );
    }

    /// Deferred padding of earlier gaussians must be materialized once a later batch does have
    /// coefficients, so that those land at the global index the shader computes.
    #[test]
    fn padding_is_flushed_before_later_coefficients() {
        let ctx = RenderContext::new_test();
        let mut builder = GaussianSplatBuilder::new(&ctx);

        builder
            .batch("no_sh")
            .add_gaussians(&centers(5), &[], &[], &[], &[], &[]);
        builder
            .batch("sh")
            .add_gaussians(&centers(2), &[], &[], &[], &coefficients(2), &[]);
        // Trailing gaussians stay deferred: nothing samples them.
        builder
            .batch("trailing_no_sh")
            .add_gaussians(&centers(3), &[], &[], &[], &[], &[]);

        // 5 zero-filled + 2 real; the trailing 3 are never uploaded.
        assert_eq!(sh_gaussians(&builder), 7);
        assert_eq!(builder.sh_padding_gaussians, 3);
        assert_eq!(builder.position_scale_x_buffer.len(), 10);
    }
}
