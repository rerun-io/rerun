use super::super::wire::Ros1Reader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Time {
    pub sec: u32,
    pub nsec: u32,
}

impl Time {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            sec: reader.read_u32()?,
            nsec: reader.read_u32()?,
        })
    }

    pub fn as_nanos(&self) -> u64 {
        u64::from(self.sec) * 1_000_000_000 + u64::from(self.nsec)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub seq: u32,
    pub stamp: Time,
    pub frame_id: String,
}

impl Header {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            seq: reader.read_u32()?,
            stamp: Time::read(reader)?,
            frame_id: reader.read_string()?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringMessage {
    pub data: String,
}

impl StringMessage {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        Ok(Self {
            data: reader.read_string()?,
        })
    }
}
