use super::super::wire::Ros1Reader;
use super::geometry_msgs::Pose;
use super::std_msgs::{Header, Time};

#[derive(Debug, Clone)]
pub struct MapMetaData {
    pub map_load_time: Time,
    pub resolution: f32,
    pub width: u32,
    pub height: u32,
    pub origin: Pose,
}

impl MapMetaData {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            map_load_time: Time::read(reader)?,
            resolution: reader.read_f32()?,
            width: reader.read_u32()?,
            height: reader.read_u32()?,
            origin: Pose::read(reader)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct OccupancyGrid {
    pub header: Header,
    pub info: MapMetaData,
    pub data: Vec<u8>,
}

impl OccupancyGrid {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            header: Header::read(reader)?,
            info: MapMetaData::read(reader)?,
            data: reader.read_i8_vec_as_u8()?,
        })
    }
}
