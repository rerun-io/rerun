use super::Uuid;

impl From<Uuid> for uuid::Uuid {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        uuid::Uuid::from_bytes(uuid.bytes)
    }
}

impl From<uuid::Uuid> for Uuid {
    #[inline]
    fn from(uuid: uuid::Uuid) -> Self {
        Self {
            bytes: *uuid.as_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_uuid() {
        let uuid = uuid::Uuid::new_v4();

        let uuid_datatype: super::Uuid = uuid.into();

        let uuid_roundtrip: uuid::Uuid = uuid_datatype.into();
        assert_eq!(uuid, uuid_roundtrip);
    }
}
