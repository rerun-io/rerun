# re_tf

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_tf.svg)](https://crates.io/crates/store/re_tf)
[![Documentation](https://docs.rs/re_tf/badge.svg)](https://docs.rs/re_tf)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Rerun's spatial transform processing.

Responsible for collecting Rerun compliant spatial transform data & processing them for higher level transform related queries.
This crate encapsulates a lot of the rules underpinning transform related datastructures defined in `re_sdk_types`.

Maintains time dependent topological data structures that allow resolving affine transformations between different transform frames (points of reference).

The name is borrowed from ROS's popular [`tf`](https://wiki.ros.org/tf) package as it plays a similar role.
