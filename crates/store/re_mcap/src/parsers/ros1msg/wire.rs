use anyhow::{Context as _, ensure};

pub struct Ros1Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Ros1Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    pub fn finish(&self) -> anyhow::Result<()> {
        ensure!(
            self.offset == self.data.len(),
            "ROS1 payload has {} trailing bytes",
            self.data.len() - self.offset
        );
        Ok(())
    }

    fn take<const N: usize>(&mut self) -> anyhow::Result<[u8; N]> {
        let end = self
            .offset
            .checked_add(N)
            .context("ROS1 read offset overflow")?;
        ensure!(
            end <= self.data.len(),
            "ROS1 payload ended while reading {N} bytes at offset {}",
            self.offset
        );
        let bytes = self.data[self.offset..end].try_into()?;
        self.offset = end;
        Ok(bytes)
    }

    fn take_slice(&mut self, len: usize) -> anyhow::Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(len)
            .context("ROS1 read offset overflow")?;
        ensure!(
            end <= self.data.len(),
            "ROS1 payload ended while reading {len} bytes at offset {}",
            self.offset
        );
        let slice = &self.data[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    pub fn read_bool(&mut self) -> anyhow::Result<bool> {
        Ok(self.read_u8()? != 0)
    }

    pub fn read_u8(&mut self) -> anyhow::Result<u8> {
        Ok(self.take::<1>()?[0])
    }

    pub fn read_u32(&mut self) -> anyhow::Result<u32> {
        Ok(u32::from_le_bytes(self.take()?))
    }

    pub fn read_f32(&mut self) -> anyhow::Result<f32> {
        Ok(f32::from_le_bytes(self.take()?))
    }

    pub fn read_f64(&mut self) -> anyhow::Result<f64> {
        Ok(f64::from_le_bytes(self.take()?))
    }

    pub fn read_string(&mut self) -> anyhow::Result<String> {
        let len = self.read_u32()? as usize;
        let bytes = self.take_slice(len)?;
        String::from_utf8(bytes.to_vec()).context("ROS1 string is not valid UTF-8")
    }

    pub fn read_u8_vec(&mut self) -> anyhow::Result<Vec<u8>> {
        let len = self.read_u32()? as usize;
        Ok(self.take_slice(len)?.to_vec())
    }

    pub fn read_i8_vec_as_u8(&mut self) -> anyhow::Result<Vec<u8>> {
        let len = self.read_u32()? as usize;
        Ok(self.take_slice(len)?.to_vec())
    }

    pub fn read_f64_vec(&mut self) -> anyhow::Result<Vec<f64>> {
        let len = self.read_u32()? as usize;
        (0..len).map(|_| self.read_f64()).collect()
    }

    pub fn read_f64_array<const N: usize>(&mut self) -> anyhow::Result<[f64; N]> {
        let values = (0..N)
            .map(|_| self.read_f64())
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(values.try_into().expect("fixed-size array length matches"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_string_and_primitives() {
        let mut bytes = Vec::new();
        bytes.extend(7_u32.to_le_bytes());
        bytes.extend(1.5_f32.to_le_bytes());
        bytes.extend(3_u32.to_le_bytes());
        bytes.extend(b"map");

        let mut reader = Ros1Reader::new(&bytes);
        assert_eq!(reader.read_u32().unwrap(), 7);
        assert_eq!(reader.read_f32().unwrap(), 1.5);
        assert_eq!(reader.read_string().unwrap(), "map");
        reader.finish().unwrap();
    }

    #[test]
    fn rejects_short_buffers() {
        let mut reader = Ros1Reader::new(&[1, 2, 3]);
        assert!(reader.read_u32().is_err());
    }

    #[test]
    fn rejects_length_overrun() {
        let bytes = 4_u32.to_le_bytes();
        let mut reader = Ros1Reader::new(&bytes);
        assert!(reader.read_string().is_err());
    }
}
