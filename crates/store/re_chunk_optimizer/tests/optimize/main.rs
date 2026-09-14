//! Integration tests: [`re_chunk_optimizer::optimize`] over an
//! [`re_log_encoding::InMemoryChunkProvider`] or an [`re_log_encoding::RrdChunkProvider`].
//! One module per theme; [`helpers`] holds the shared fixtures.

#![expect(clippy::unwrap_used)] // `allow-unwrap-in-tests` does not cover helpers in `tests/`

mod helpers;

mod end_to_end;
mod merge;
mod own_chunk;
mod split;
mod unsorted;
