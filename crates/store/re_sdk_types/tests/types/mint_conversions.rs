use re_sdk_types::{components, datatypes};

#[test]
fn vec2d() {
    {
        let datatype: datatypes::Vec2D = [1.0, 2.0].into();
        let mint: mint::Vector2<f32> = datatype.into();
        assert_eq!(mint, [1.0, 2.0].into());
    }
    {
        let mint: mint::Vector2<f32> = [1.0, 2.0].into();
        let datatype: datatypes::Vec2D = mint.into();
        assert_eq!(datatype.x(), 1.0);
        assert_eq!(datatype.y(), 2.0);
    }
}

#[test]
fn vec3d() {
    {
        let datatype: datatypes::Vec3D = [1.0, 2.0, 3.0].into();
        let mint: mint::Vector3<f32> = datatype.into();
        assert_eq!(mint, [1.0, 2.0, 3.0].into());
    }
    {
        let mint: mint::Vector3<f32> = [1.0, 2.0, 3.0].into();
        let datatype: datatypes::Vec3D = mint.into();
        assert_eq!(datatype.x(), 1.0);
        assert_eq!(datatype.y(), 2.0);
        assert_eq!(datatype.z(), 3.0);
    }
}

#[test]
fn vec4d() {
    {
        let datatype: datatypes::Vec4D = [1.0, 2.0, 3.0, 4.0].into();
        let mint: mint::Vector4<f32> = datatype.into();
        assert_eq!(mint.x, 1.0);
        assert_eq!(mint.y, 2.0);
        assert_eq!(mint.z, 3.0);
        assert_eq!(mint.w, 4.0);
    }
    {
        let mint: mint::Vector4<f32> = [1.0, 2.0, 3.0, 4.0].into();
        let datatype: datatypes::Vec4D = mint.into();
        assert_eq!(datatype.x(), 1.0);
        assert_eq!(datatype.y(), 2.0);
        assert_eq!(datatype.z(), 3.0);
        assert_eq!(datatype.w(), 4.0);
    }
}

#[test]
fn position2d() {
    {
        let component: components::Position2D = [1.0, 2.0].into();
        let mint: mint::Point2<f32> = component.into();
        assert_eq!(mint.x, 1.0);
        assert_eq!(mint.y, 2.0);
    }
    {
        let mint: mint::Point2<f32> = [1.0, 2.0].into();
        let component: components::Position2D = mint.into();
        assert_eq!(component.x(), 1.0);
        assert_eq!(component.y(), 2.0);
    }
}

#[test]
fn position3d() {
    {
        let component: components::Position3D = [1.0, 2.0, 3.0].into();
        let mint: mint::Point3<f32> = component.into();
        assert_eq!(mint, [1.0, 2.0, 3.0].into());
    }
    {
        let mint: mint::Point3<f32> = [1.0, 2.0, 3.0].into();
        let component: components::Position3D = mint.into();
        assert_eq!(component.x(), 1.0);
        assert_eq!(component.y(), 2.0);
        assert_eq!(component.z(), 3.0);
    }
}

#[test]
fn half_sizes_2d() {
    {
        let component: components::HalfSize2D = [1.0, 2.0].into();
        let mint: mint::Vector2<f32> = component.into();
        assert_eq!(mint, [1.0, 2.0].into());
    }
    {
        let mint: mint::Vector2<f32> = [1.0, 2.0].into();
        let component: components::HalfSize2D = mint.into();
        assert_eq!(component.x(), 1.0);
        assert_eq!(component.y(), 2.0);
    }
}

#[test]
fn half_sizes_3d() {
    {
        let component: components::HalfSize3D = [1.0, 2.0, 3.0].into();
        let mint: mint::Vector3<f32> = component.into();
        assert_eq!(mint, [1.0, 2.0, 3.0].into());
    }
    {
        let mint: mint::Vector3<f32> = [1.0, 2.0, 3.0].into();
        let component: components::HalfSize3D = mint.into();
        assert_eq!(component.x(), 1.0);
        assert_eq!(component.y(), 2.0);
        assert_eq!(component.z(), 3.0);
    }
}

#[test]
fn quaternion() {
    {
        let datatype = datatypes::Quaternion::from_xyzw([1.0, 2.0, 3.0, 4.0]);
        let mint: mint::Quaternion<f32> = datatype.into();
        assert_eq!(mint, [1.0, 2.0, 3.0, 4.0].into());
    }
    {
        let mint: mint::Quaternion<f32> = [1.0, 2.0, 3.0, 4.0].into();
        let datatype: datatypes::Quaternion = mint.into();
        assert_eq!(datatype.0[0], 1.0);
        assert_eq!(datatype.0[1], 2.0);
        assert_eq!(datatype.0[2], 3.0);
        assert_eq!(datatype.0[3], 4.0);
    }
}

#[test]
fn mat3() {
    let m = [
        [1.0, 2.0, 3.0], //
        [4.0, 5.0, 6.0], //
        [7.0, 8.0, 9.0], //
    ];

    {
        let datatype: datatypes::Mat3x3 = m.into();
        let mint: mint::ColumnMatrix3<f32> = datatype.into();
        assert_eq!(mint, m.into());
    }
    {
        let mint: mint::ColumnMatrix3<f32> = m.into();
        let datatype: datatypes::Mat3x3 = mint.into();
        assert_eq!(datatype, m.into());
    }
}

#[test]
fn mat4() {
    let m = [
        [0.0, 1.0, 2.0, 3.0],
        [4.0, 5.0, 6.0, 7.0],
        [8.0, 9.0, 10.0, 11.0],
        [12.0, 13.0, 14.0, 15.0],
    ];

    {
        let datatype: datatypes::Mat4x4 = m.into();
        let mint: mint::ColumnMatrix4<f32> = datatype.into();
        assert_eq!(mint, m.into());
    }
    {
        let mint: mint::ColumnMatrix4<f32> = m.into();
        let datatype: datatypes::Mat4x4 = mint.into();
        assert_eq!(datatype, m.into());
    }
}
