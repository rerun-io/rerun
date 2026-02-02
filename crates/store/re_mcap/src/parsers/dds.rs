//! # DDS-RTPS Representation Identifiers
//!
//! This module provides types for working with DDS-RTPS representation identifiers
//! as defined in Section 10.2 of the [DDS-RTPS 2.3 specification](https://www.omg.org/spec/DDSI-RTPS/2.3/PDF).
//!
//! Representation identifiers specify how data payloads are encoded in DDS-RTPS messages.
//! Each identifier is two bytes: the first is always `0x00`, the second selects the format.
//!
//! ## Supported Formats
//!
//! - **CDR v1**: Classic CDR with big/little endian variants (`0x00`, `0x01`)
//! - **Parameter Lists**: Parameter list format with endian variants (`0x02`, `0x03`)
//! - **XML**: XML representation (`0x04`)
//! - **CDR v2**: Extended CDR formats for plain, mutable, and appendable types (`0x10`-`0x15`)

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DdsError {
    #[error("Unknown representation identifier `{0:?}`")]
    UnknownIdentifier([u8; 2]),

    #[error("Invalid first byte got `{0}`, but it should always be `0x00`")]
    InvalidFirstByte(u8),
}

/// A DDS-RTPS representation identifier as defined in Section 10.2 of
/// the [DDS-RTPS 2.3 specification](https://www.omg.org/spec/DDSI-RTPS/2.3/PDF).
///
/// Each identifier is encoded as two bytes:
/// - The first byte is always`0x00` for the currently supported representations.
/// - The second byte selects one of the supported representations.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepresentationIdentifier {
    /// Classic CDR representation with Big Endian encoding
    CdrBigEndian = 0x00,

    /// Classic CDR representation with Little Endian encoding
    CdrLittleEndian = 0x01,

    /// Parameter list with Big Endian encoding
    ParameterListBigEndian = 0x02,

    /// Parameter list with Little Endian encoding
    ParameterListLittleEndian = 0x03,

    /// XML representation
    Xml = 0x04,

    /// Plain CDR representation (version2) with Big Endian encoding
    Cdr2BigEndian = 0x10,

    /// Plain CDR representation (version2) with Little Endian encoding
    Cdr2LittleEndian = 0x11,

    /// Extended CDR representation (version2) for MUTABLE types with Big Endian encoding
    ParameterListCdr2BigEndian = 0x12,

    /// Extended CDR representation (version2) for MUTABLE types with Little Endian encoding
    ParameterListCdr2LittleEndian = 0x13,

    /// Extended CDR representation (version2) for APPENDABLE types with Big Endian encoding
    DelimitedCdrBigEndian = 0x14,

    /// Extended CDR representation (version2) for APPENDABLE types with Little Endian encoding
    DelimitedCdrLittleEndian = 0x15,
}

impl RepresentationIdentifier {
    /// Creates a [`RepresentationIdentifier`] from two bytes.
    pub fn from_bytes(bytes: [u8; 2]) -> Result<Self, DdsError> {
        if bytes[0] != 0x00 {
            return Err(DdsError::InvalidFirstByte(bytes[0]));
        }

        Self::from_second_byte(bytes[1]).ok_or(DdsError::UnknownIdentifier(bytes))
    }

    /// Creates a [`RepresentationIdentifier`] from just the second byte, assuming first byte is `0x00`.
    pub fn from_second_byte(second_byte: u8) -> Option<Self> {
        // Mappings taken from Table 10.3 of the [DDS-RTPS 2.3 specification](https://www.omg.org/spec/DDSI-RTPS/2.3/PDF).
        match second_byte {
            0x00 => Some(Self::CdrBigEndian),
            0x01 => Some(Self::CdrLittleEndian),
            0x02 => Some(Self::ParameterListBigEndian),
            0x03 => Some(Self::ParameterListLittleEndian),
            0x04 => Some(Self::Xml),
            0x10 => Some(Self::Cdr2BigEndian),
            0x11 => Some(Self::Cdr2LittleEndian),
            0x12 => Some(Self::ParameterListCdr2BigEndian),
            0x13 => Some(Self::ParameterListCdr2LittleEndian),
            0x14 => Some(Self::DelimitedCdrBigEndian),
            0x15 => Some(Self::DelimitedCdrLittleEndian),
            _ => None,
        }
    }

    /// Converts the [`RepresentationIdentifier`] to bytes.
    pub fn to_bytes(self) -> [u8; 2] {
        [0x00, self as u8]
    }

    /// Returns `true` if this representation uses big endian encoding.
    pub fn is_big_endian(self) -> bool {
        matches!(
            self,
            Self::CdrBigEndian
                | Self::ParameterListBigEndian
                | Self::Cdr2BigEndian
                | Self::ParameterListCdr2BigEndian
                | Self::DelimitedCdrBigEndian
        )
    }

    /// Returns `true` if this representation uses little endian encoding.
    pub fn is_little_endian(self) -> bool {
        matches!(
            self,
            Self::CdrLittleEndian
                | Self::ParameterListLittleEndian
                | Self::Cdr2LittleEndian
                | Self::ParameterListCdr2LittleEndian
                | Self::DelimitedCdrLittleEndian
        )
    }

    /// Returns true if this representation uses CDR format (version 1)
    pub fn is_cdr(self) -> bool {
        matches!(self, Self::CdrBigEndian | Self::CdrLittleEndian)
    }

    /// Returns true if this representation uses CDR2 format (version 2)
    pub fn is_cdr2(self) -> bool {
        matches!(
            self,
            Self::Cdr2BigEndian
                | Self::Cdr2LittleEndian
                | Self::ParameterListCdr2BigEndian
                | Self::ParameterListCdr2LittleEndian
                | Self::DelimitedCdrBigEndian
                | Self::DelimitedCdrLittleEndian
        )
    }

    /// Returns true if this representation uses `ParameterList` format (version 1)
    pub fn is_parameter_list(self) -> bool {
        matches!(
            self,
            Self::ParameterListBigEndian | Self::ParameterListLittleEndian
        )
    }

    /// Returns true if this representation uses `ParameterList` format for CDR2 (MUTABLE types)
    pub fn is_parameter_list_cdr2(self) -> bool {
        matches!(
            self,
            Self::ParameterListCdr2BigEndian | Self::ParameterListCdr2LittleEndian
        )
    }

    /// Returns true if this representation uses Delimited CDR format (APPENDABLE types)
    pub fn is_delimited_cdr(self) -> bool {
        matches!(
            self,
            Self::DelimitedCdrBigEndian | Self::DelimitedCdrLittleEndian
        )
    }

    /// Returns true if this representation uses XML format
    pub fn is_xml(self) -> bool {
        matches!(self, Self::Xml)
    }

    /// Returns true if this representation has endianness (XML does not)
    pub fn has_endianness(self) -> bool {
        !self.is_xml()
    }
}

// Convenience implementations for common conversions
impl TryFrom<[u8; 2]> for RepresentationIdentifier {
    type Error = DdsError;

    fn try_from(bytes: [u8; 2]) -> Result<Self, Self::Error> {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<u8> for RepresentationIdentifier {
    type Error = u8;

    fn try_from(second_byte: u8) -> Result<Self, Self::Error> {
        Self::from_second_byte(second_byte).ok_or(second_byte)
    }
}

impl From<RepresentationIdentifier> for [u8; 2] {
    fn from(repr: RepresentationIdentifier) -> Self {
        repr.to_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_bytes() {
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x00]).unwrap(),
            RepresentationIdentifier::CdrBigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x01]).unwrap(),
            RepresentationIdentifier::CdrLittleEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x02]).unwrap(),
            RepresentationIdentifier::ParameterListBigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x03]).unwrap(),
            RepresentationIdentifier::ParameterListLittleEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x04]).unwrap(),
            RepresentationIdentifier::Xml
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x10]).unwrap(),
            RepresentationIdentifier::Cdr2BigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x11]).unwrap(),
            RepresentationIdentifier::Cdr2LittleEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x12]).unwrap(),
            RepresentationIdentifier::ParameterListCdr2BigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x13]).unwrap(),
            RepresentationIdentifier::ParameterListCdr2LittleEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x14]).unwrap(),
            RepresentationIdentifier::DelimitedCdrBigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_bytes([0x00, 0x15]).unwrap(),
            RepresentationIdentifier::DelimitedCdrLittleEndian
        );
    }

    #[test]
    fn test_from_second_byte() {
        assert_eq!(
            RepresentationIdentifier::from_second_byte(0x00).unwrap(),
            RepresentationIdentifier::CdrBigEndian
        );
        assert_eq!(
            RepresentationIdentifier::from_second_byte(0x04).unwrap(),
            RepresentationIdentifier::Xml
        );
        assert_eq!(
            RepresentationIdentifier::from_second_byte(0x15).unwrap(),
            RepresentationIdentifier::DelimitedCdrLittleEndian
        );
        assert!(RepresentationIdentifier::from_second_byte(0xFF).is_none());
    }

    #[test]
    fn test_invalid_first_byte() {
        let result = RepresentationIdentifier::from_bytes([0x01, 0x00]);
        assert!(matches!(result, Err(DdsError::InvalidFirstByte(0x01))));
    }

    #[test]
    fn test_unknown_identifier() {
        let result = RepresentationIdentifier::from_bytes([0x00, 0xFF]);
        assert!(matches!(
            result,
            Err(DdsError::UnknownIdentifier([0x00, 0xFF]))
        ));
    }

    #[test]
    fn test_to_bytes() {
        assert_eq!(
            RepresentationIdentifier::CdrBigEndian.to_bytes(),
            [0x00, 0x00]
        );
        assert_eq!(
            RepresentationIdentifier::CdrLittleEndian.to_bytes(),
            [0x00, 0x01]
        );
        assert_eq!(
            RepresentationIdentifier::ParameterListBigEndian.to_bytes(),
            [0x00, 0x02]
        );
        assert_eq!(
            RepresentationIdentifier::ParameterListLittleEndian.to_bytes(),
            [0x00, 0x03]
        );
        assert_eq!(RepresentationIdentifier::Xml.to_bytes(), [0x00, 0x04]);
        assert_eq!(
            RepresentationIdentifier::Cdr2BigEndian.to_bytes(),
            [0x00, 0x10]
        );
        assert_eq!(
            RepresentationIdentifier::Cdr2LittleEndian.to_bytes(),
            [0x00, 0x11]
        );
        assert_eq!(
            RepresentationIdentifier::ParameterListCdr2BigEndian.to_bytes(),
            [0x00, 0x12]
        );
        assert_eq!(
            RepresentationIdentifier::ParameterListCdr2LittleEndian.to_bytes(),
            [0x00, 0x13]
        );
        assert_eq!(
            RepresentationIdentifier::DelimitedCdrBigEndian.to_bytes(),
            [0x00, 0x14]
        );
        assert_eq!(
            RepresentationIdentifier::DelimitedCdrLittleEndian.to_bytes(),
            [0x00, 0x15]
        );
    }

    #[test]
    fn test_endianness_checks() {
        // Big endian variants
        assert!(RepresentationIdentifier::CdrBigEndian.is_big_endian());
        assert!(!RepresentationIdentifier::CdrBigEndian.is_little_endian());
        assert!(RepresentationIdentifier::ParameterListBigEndian.is_big_endian());
        assert!(RepresentationIdentifier::Cdr2BigEndian.is_big_endian());
        assert!(RepresentationIdentifier::ParameterListCdr2BigEndian.is_big_endian());
        assert!(RepresentationIdentifier::DelimitedCdrBigEndian.is_big_endian());

        // Little endian variants
        assert!(!RepresentationIdentifier::CdrLittleEndian.is_big_endian());
        assert!(RepresentationIdentifier::CdrLittleEndian.is_little_endian());
        assert!(RepresentationIdentifier::ParameterListLittleEndian.is_little_endian());
        assert!(RepresentationIdentifier::Cdr2LittleEndian.is_little_endian());
        assert!(RepresentationIdentifier::ParameterListCdr2LittleEndian.is_little_endian());
        assert!(RepresentationIdentifier::DelimitedCdrLittleEndian.is_little_endian());

        // XML has no endianness
        assert!(!RepresentationIdentifier::Xml.is_big_endian());
        assert!(!RepresentationIdentifier::Xml.is_little_endian());
        assert!(!RepresentationIdentifier::Xml.has_endianness());
    }
}
