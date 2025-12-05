use super::Capsules3D;
use crate::components;

impl Capsules3D {
    /// Creates a new [`Capsules3D`] with the given axis-aligned lengths and radii.
    ///
    /// For multiple capsules, you should generally follow this with
    /// [`Capsules3D::with_translations()`] and one of the rotation methods, in order to move them
    /// apart from each other.
    pub fn from_lengths_and_radii(
        lengths: impl IntoIterator<Item = impl Into<components::Length>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        Self::new(lengths, radii)
    }

    /// Creates a new [`Capsules3D`] where each capsule extends between the given pairs of points.
    #[cfg(feature = "glam")]
    pub fn from_endpoints_and_radii(
        start_points: impl IntoIterator<Item = impl Into<components::Position3D>>,
        end_points: impl IntoIterator<Item = impl Into<components::Position3D>>,
        radii: impl IntoIterator<Item = f32>,
    ) -> Self {
        use itertools::Itertools as _; // for .multiunzip()

        let (lengths, translations, quaternions): (
            Vec<components::Length>,
            Vec<components::Translation3D>,
            Vec<components::RotationQuat>,
        ) = start_points
            .into_iter()
            .zip(end_points)
            .map(|(p1, p2)| {
                let p1: glam::Vec3 = p1.into().0.into();
                let p2: glam::Vec3 = p2.into().0.into();

                // Convert the pair of points to the distance between them and the rotation
                // from +Z to the direction between them.
                let direction = p2 - p1;

                if let Some(normalized_direction) = direction.try_normalize() {
                    (
                        components::Length::from(direction.length()),
                        components::Translation3D::from(p1),
                        components::RotationQuat::from(glam::Quat::from_rotation_arc(
                            glam::Vec3::Z,
                            normalized_direction,
                        )),
                    )
                } else {
                    (
                        components::Length::from(0.0),
                        components::Translation3D::from(p1),
                        components::RotationQuat::IDENTITY,
                    )
                }
            })
            .multiunzip();

        Self::new(lengths, radii)
            .with_translations(translations)
            .with_quaternions(quaternions)
    }
}

#[cfg(test)]
#[cfg(feature = "glam")]
mod tests {
    use glam::vec3;

    use super::*;

    #[test]
    fn endpoints_equivalent_to_rotation() {
        // Very luckily, the math works out exactly in this test.
        // If this ever fails due to rounding error, we'll have to make it messier by adding
        // a comparison with allowed error.
        let radius = 0.25;
        let length = 2.;
        let endpoint_1 = vec3(-1., 1., 0.);
        let endpoint_2 = endpoint_1 + glam::Vec3::X * length;
        assert_eq!(
            Capsules3D::from_endpoints_and_radii([endpoint_1], [endpoint_2], [radius]),
            Capsules3D::from_lengths_and_radii([length], [radius])
                .with_translations([endpoint_1])
                .with_quaternions([
                    // rotate 90° about Y to rotate the +Z capsule into +X
                    glam::Quat::from_axis_angle(vec3(0., 1., 0.), std::f32::consts::FRAC_PI_2)
                ])
        );
    }

    #[test]
    fn endpoints_zero_length() {
        let endpoint = vec3(1., 2., 3.);
        let radius = 0.25;
        assert_eq!(
            Capsules3D::from_endpoints_and_radii([endpoint], [endpoint], [radius]),
            Capsules3D::from_lengths_and_radii([0.0], [radius])
                .with_translations([endpoint])
                .with_quaternions([glam::Quat::IDENTITY])
        );
    }
}
