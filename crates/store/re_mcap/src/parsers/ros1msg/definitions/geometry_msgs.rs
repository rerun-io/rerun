use super::super::wire::Ros1Reader;
use super::std_msgs::Header;

#[derive(Debug, Clone)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            x: reader.read_f64()?,
            y: reader.read_f64()?,
            z: reader.read_f64()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

impl Quaternion {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            x: reader.read_f64()?,
            y: reader.read_f64()?,
            z: reader.read_f64()?,
            w: reader.read_f64()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Point {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            x: reader.read_f64()?,
            y: reader.read_f64()?,
            z: reader.read_f64()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Pose {
    pub position: Point,
    pub orientation: Quaternion,
}

impl Pose {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            position: Point::read(reader)?,
            orientation: Quaternion::read(reader)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Transform {
    pub translation: Vector3,
    pub rotation: Quaternion,
}

impl Transform {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            translation: Vector3::read(reader)?,
            rotation: Quaternion::read(reader)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct TransformStamped {
    pub header: Header,
    pub child_frame_id: String,
    pub transform: Transform,
}

impl TransformStamped {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            header: Header::read(reader)?,
            child_frame_id: reader.read_string()?,
            transform: Transform::read(reader)?,
        })
    }
}
