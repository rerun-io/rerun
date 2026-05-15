use super::super::wire::Ros1Reader;
use super::std_msgs::Header;

#[derive(Debug, Clone)]
pub struct Image {
    pub header: Header,
    pub height: u32,
    pub width: u32,
    pub encoding: String,
    pub is_bigendian: u8,
    pub step: u32,
    pub data: Vec<u8>,
}

impl Image {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            header: Header::read(reader)?,
            height: reader.read_u32()?,
            width: reader.read_u32()?,
            encoding: reader.read_string()?,
            is_bigendian: reader.read_u8()?,
            step: reader.read_u32()?,
            data: reader.read_u8_vec()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CompressedImage {
    pub header: Header,
    pub format: String,
    pub data: Vec<u8>,
}

impl CompressedImage {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            header: Header::read(reader)?,
            format: reader.read_string()?,
            data: reader.read_u8_vec()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct RegionOfInterest {
    pub x_offset: u32,
    pub y_offset: u32,
    pub height: u32,
    pub width: u32,
    pub do_rectify: bool,
}

impl RegionOfInterest {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            x_offset: reader.read_u32()?,
            y_offset: reader.read_u32()?,
            height: reader.read_u32()?,
            width: reader.read_u32()?,
            do_rectify: reader.read_bool()?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CameraInfo {
    pub header: Header,
    pub height: u32,
    pub width: u32,
    pub distortion_model: String,
    pub d: Vec<f64>,
    pub k: [f64; 9],
    pub r: [f64; 9],
    pub p: [f64; 12],
    pub binning_x: u32,
    pub binning_y: u32,
    pub roi: RegionOfInterest,
}

impl CameraInfo {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            header: Header::read(reader)?,
            height: reader.read_u32()?,
            width: reader.read_u32()?,
            distortion_model: reader.read_string()?,
            d: reader.read_f64_vec()?,
            k: reader.read_f64_array()?,
            r: reader.read_f64_array()?,
            p: reader.read_f64_array()?,
            binning_x: reader.read_u32()?,
            binning_y: reader.read_u32()?,
            roi: RegionOfInterest::read(reader)?,
        })
    }
}
