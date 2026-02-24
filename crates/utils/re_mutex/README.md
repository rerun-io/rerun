# re_mutex

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_mutex.svg)](https://crates.io/crates/re_mutex)
[![Documentation](https://docs.rs/re_mutex/badge.svg)](https://docs.rs/re_mutex)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A wrapper around [parking_log::Mutex](https://docs.rs/parking_lot/latest/parking_lot/type.Mutex.html) which logs a backtrace if a lock waits on the lock for more than 10 seconds. To make it easier to debug deadlocks.
