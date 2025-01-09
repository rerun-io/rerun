use std::fmt::Display;

use crate::Error;

/// Scopes define the context in which the token is valid.
///
/// The default scope is read-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Scope {
    write: bool,
}

impl Display for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self { write: false } => write!(f, "read"),
            Self { write: true } => write!(f, "read|write"),
        }
    }
}

impl Scope {
    pub fn read_only() -> Self {
        Self { write: false }
    }

    pub fn read_write() -> Self {
        Self { write: true }
    }

    pub fn allows(&self, other: Self) -> Result<(), Error> {
        // This should be formalized in the future.
        if self.write && !other.write {
            Err(Error::InvalidPermission {
                expected: self.to_string(),
                actual: other.to_string(),
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_scope_display() {
        let read_only = Scope::read_only();
        let read_write = Scope::read_write();

        assert_eq!(read_only.to_string(), "read");
        assert_eq!(read_write.to_string(), "read|write");
    }

    #[test]
    fn test_scope_allows() {
        let read_only = Scope::read_only();
        let read_write = Scope::read_write();

        assert!(read_only.allows(read_only).is_ok());
        assert!(read_only.allows(read_write).is_ok());

        assert!(read_write.allows(read_write).is_ok());
        assert!(read_write.allows(read_only).is_err());
    }
}
