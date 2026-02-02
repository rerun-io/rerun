use super::Uuid;

impl Uuid {
    /// Generate a new random UUID.
    #[inline]
    pub fn random() -> Self {
        Self {
            bytes: *uuid::Uuid::new_v4().as_bytes(),
        }
    }
}

impl From<Uuid> for uuid::Uuid {
    #[inline]
    fn from(uuid: Uuid) -> Self {
        Self::from_bytes(uuid.bytes)
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

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        uuid::Uuid::from(*self).fmt(f)
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
