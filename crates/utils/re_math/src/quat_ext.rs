use glam::{Quat, Vec3};

/// Extensions to [`Quat`]
pub trait QuatExt: Sized {
    /// Return a Quaternion that rotates -Z to the `forward` direction,
    /// using `up` to control roll, so that +Y will approximately point in the `up` direction.
    ///
    /// Will return [`None`] if either argument is zero, non-finite, or if they are colinear.
    ///
    /// This is generally what you want to use to construct a view-rotation when -Z is forward and +Y is up (this is what Ark uses!).
    fn rotate_negative_z_towards(forward: Vec3, up: Vec3) -> Option<Quat>;

    /// Return a Quaternion that rotates +Z to the `forward` direction,
    /// using `up` to control roll, so that +Y will approximately point in the `up` direction.
    ///
    /// Will return [`None`] if either argument is zero, non-finite, or if they are colinear.
    ///
    /// This is generally what you want to use to construct a view-rotation when +Z is forward and +Y is up.
    fn rotate_positive_z_towards(forward: Vec3, up: Vec3) -> Option<Quat>;
}

impl QuatExt for Quat {
    fn rotate_negative_z_towards(forward: Vec3, up: Vec3) -> Option<Quat> {
        let forward = forward.normalize_or_zero();
        let side = forward.cross(up).normalize_or_zero(); // `side` is right in a right-handed system
        let up = side.cross(forward);

        if forward != Vec3::ZERO && side != Vec3::ZERO && up != Vec3::ZERO {
            Some(Self::from_mat3(&glam::Mat3::from_cols(side, up, -forward)))
        } else {
            None
        }
    }

    fn rotate_positive_z_towards(forward: Vec3, up: Vec3) -> Option<Quat> {
        let forward = forward.normalize_or_zero();
        let side = up.cross(forward).normalize_or_zero(); // `side` is left in a right-handed system
        let up = forward.cross(side);

        if forward != Vec3::ZERO && side != Vec3::ZERO && up != Vec3::ZERO {
            Some(Self::from_mat3(&glam::Mat3::from_cols(side, up, forward)))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_rotate_negative_z_towards() {
        #![allow(clippy::disallowed_methods)] // normalize

        let desired_fwd = Vec3::new(1.0, 2.0, 3.0).normalize();
        let desired_up = Vec3::new(4.0, 5.0, 6.0).normalize();

        let rot = Quat::rotate_negative_z_towards(desired_fwd, desired_up).unwrap();

        let rotated_z = rot * -Vec3::Z;
        assert!(
            (rotated_z - desired_fwd).length() < 1e-5,
            "Expected to rotate -Z to {desired_fwd}, but got {rotated_z}"
        );

        let rotated_y = rot * Vec3::Y;
        assert!(
            rotated_y.dot(desired_up) >= 0.0,
            "Expected to rotate +Y to point approximately towards {desired_up}, but got {rotated_y}",
        );
    }

    #[test]
    fn test_rotate_positive_z_towards() {
        #![allow(clippy::disallowed_methods)] // normalize

        let desired_fwd = Vec3::new(1.0, 2.0, 3.0).normalize();
        let desired_up = Vec3::new(4.0, 5.0, 6.0).normalize();

        let rot = Quat::rotate_positive_z_towards(desired_fwd, desired_up).unwrap();

        let rotated_z = rot * Vec3::Z;
        assert!(
            (rotated_z - desired_fwd).length() < 1e-5,
            "Expected to rotate +Z to {desired_fwd}, but got {rotated_z}",
        );

        let rotated_y = rot * Vec3::Y;
        assert!(
            rotated_y.dot(desired_up) >= 0.0,
            "Expected to rotate +Y to point approximately towards {desired_up}, but got {rotated_y}",
        );
    }
}
