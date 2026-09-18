//! The streaming dataset-query pipeline (`PIPELINE_V2.md`).
//!
//! Per DataFusion partition, one owned stream tree: a plan-driven fetch
//! executor (`buffered(N)`, issuance gated by the byte window `W`) feeds
//! per-segment processing driven by a plan-cursor watermark, with ordered
//! head-of-line emission.
//!
//! The pure internals here carry the invariants the executor relies on:
//!
//! * [`plan`] — the immutable per-partition fetch plan, ordered by
//!   `[segment, cursor key]`, which is both issuance and emission order.
//! * [`window`] — `W`, the single flow-control knob, denominated in
//!   plan-estimate bytes.
//! * [`segment`] — per-segment state, whose plan cursor yields the safe
//!   horizon that gates emission.

// TODO(tsaucer): remove this attribute once the executor/driver/bridge wires
// the module into `SegmentStreamExec::execute`; until then nothing below is
// reachable.
#![allow(dead_code)]

pub(crate) mod plan;
pub(crate) mod segment;
pub(crate) mod window;
