//! CDR (Common Data Representation) decoding utilities for DDS-RTPS messages.
//!
//! This module handles decoding CDR-encoded buffers according to the [DDS-RTPS 2.3 specification](https://www.omg.org/spec/DDSI-RTPS/2.3/PDF).
//! It automatically detects the representation format and endianness from the message header,
//! then deserializes the data accordingly.
//!
//! # Example
//!
//! ```rust
//! # use serde::Deserialize;
//! # use anyhow::Result;
//! #
//! #[derive(Deserialize)]
//! struct MyMessage {
//!     id: u32,
//!     name: String,
//! }
//!
//! # fn example() -> Result<()> {
//! # let cdr_buffer = [0u8; 8]; // dummy buffer
//! let decoded: MyMessage = re_mcap::cdr::try_decode_message(&cdr_buffer)?;
//! # Ok(())
//! # }
//! ```

use anyhow::anyhow;
use serde::Deserialize;
use thiserror::Error;

use super::dds::RepresentationIdentifier;

/// Decode a CDR-encoded DDS message into a `T`.
///
/// Expects the first 4 bytes to be:
/// 1. A 2-byte representation identifier.
/// 2. A 2-byte (unused in the specification) options field.
///
/// The rest is decoded according to the identifier’s endianness.
pub fn try_decode_message<'d, T: Deserialize<'d>>(msg: &'d [u8]) -> Result<T, CdrError> {
    if msg.len() < 4 {
        return Err(CdrError::Other(anyhow!("Invalid CDR buffer")));
    }

    let representation_identifier = RepresentationIdentifier::from_bytes([msg[0], msg[1]])?;

    // Only attempt to decode CDR messages
    if !representation_identifier.is_cdr() && !representation_identifier.is_cdr2() {
        return Err(CdrError::UnsupportedRepresentation(
            representation_identifier,
        ));
    }

    // Skip the representation identifier bytes (2), and the representation option bytes (2).
    if representation_identifier.is_big_endian() {
        cdr_encoding::from_bytes::<T, byteorder::BigEndian>(&msg[4..])
            .map(|(v, _)| v)
            .map_err(CdrError::CdrEncoding)
    } else {
        cdr_encoding::from_bytes::<T, byteorder::LittleEndian>(&msg[4..])
            .map(|(v, _)| v)
            .map_err(CdrError::CdrEncoding)
    }
}

/// Errors from CDR decoding.
#[derive(Error, Debug)]
pub enum CdrError {
    #[error("Failed to deserialize CDR-encoded message: {0}")]
    CdrEncoding(#[from] cdr_encoding::Error),

    #[error("Failed to parse DDS message: {0}")]
    Dds(#[from] super::dds::DdsError),

    #[error("Message is not encoded using a CDR representation: `{0:?}`")]
    UnsupportedRepresentation(RepresentationIdentifier),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}
