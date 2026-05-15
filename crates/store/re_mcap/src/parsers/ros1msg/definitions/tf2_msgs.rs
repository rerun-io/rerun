use super::super::wire::Ros1Reader;
use super::geometry_msgs::TransformStamped;

#[derive(Debug, Clone)]
pub struct TFMessage {
    pub transforms: Vec<TransformStamped>,
}

impl TFMessage {
    pub fn read(reader: &mut Ros1Reader<'_>) -> anyhow::Result<Self> {
        let len = reader.read_u32()? as usize;
        Ok(Self {
            transforms: (0..len)
                .map(|_| TransformStamped::read(reader))
                .collect::<anyhow::Result<_>>()?,
        })
    }
}
