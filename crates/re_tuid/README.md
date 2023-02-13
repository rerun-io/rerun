# TUID: Time-based Unique IDentifier

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_tuid.svg)](https://crates.io/crates/re_tuid)
[![Documentation](https://docs.rs/re_tuid/badge.svg)](https://docs.rs/re_tuid)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

TUID:s are 128-bit identifiers, that have a global time-based order, with tie-breaking between threads. This means you can use a TUID as a tie-breaker in time series databases.

## Implementation
TUID is based on two fields, both of which are monotonically increasing:

* `time_ns: u64`
* `inc: u64`

`time_ns` is an approximate nanoseconds since unix epoch. It is monotonically increasing, though two TUID:s generated closely together may get the same `time_ns`.

`inc` is a monotonically increasing integer, initialized to some random number on each thread.

So the algorithm is this:

* For each thread, generate a 64-bit random number as `inc`
* When generating a new TUID:
    * increment the thread-local `inc`
    * get current time as `time_ns`
    * return `TUID { time_ns, inc }`

## Performance
On a single core of a 2022 M1 MacBook we can generate 40 million TUID/s, which is 25 ns per TUID.

## Future work
For time-based exploits (like Meltdown/Spectre) `time_ns` should probably be rounded to nearest millisecond for sensitive systems. The last ~20 bits of `time_ns` can be filled with more randomness to lessen the chance of collisions.
