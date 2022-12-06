use arrow2_convert::ArrowField;

#[derive(Debug, PartialEq, ArrowField)]
pub struct Rect2D {
    /// Rect X-coordinate
    pub x: f32,
    /// Rect Y-coordinate
    pub y: f32,
    /// Box Width
    pub w: f32,
    /// Box Height
    pub h: f32,
}

#[derive(Debug, PartialEq, ArrowField)]
pub struct Point2D {
    x: f32,
    y: f32,
}

#[derive(Debug, PartialEq, ArrowField)]
pub struct Point3D {
    x: f32,
    y: f32,
    z: f32,
}

#[allow(dead_code)]
pub type ColorRGBA = u32;
