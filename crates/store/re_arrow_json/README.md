# re_arrow_json

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_arrow_json.svg)](https://crates.io/crates/re_arrow_json)
[![Documentation](https://docs.rs/re_arrow_json/badge.svg)](https://docs.rs/re_arrow_json)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Rerun data as JSON, and back.

An entity is one JSON object holding one key per archetype, and one key per component of that archetype.
Component values are encoded from their Arrow datatype with `arrow-json`, with friendlier forms for enums, colors, UUIDs and unions.
The viewer uses it to read and write blueprints over `ViewerControl`.
