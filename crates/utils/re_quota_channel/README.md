# re_quota_channel

Part of the [Rerun](https://github.com/rerun-io/rerun) project.

[![Latest version](https://img.shields.io/crates/v/re_capabilities.svg)](https://crates.io/crates/re_capabilities)
[![Documentation](https://docs.rs/re_capabilities/badge.svg)](https://docs.rs/re_capabilities)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

A mpsc channel that applies backpressure based on byte size.

## Overview

This crate provides a multi-producer, single-consumer channel that limits throughput based on
the total byte size of messages in the channel, rather than just the number of messages.

When the byte capacity is exceeded:
- **Native platforms**: The `send` method blocks until space is available
- **WebAssembly**: Blocking is not possible, so a warning is logged and the message is sent anyway

## Usage

```rust
use re_quota_channel::channel;

// Create a channel with 1MB capacity
let (tx, rx) = channel::<Vec<u8>>("my_channel", 1_000_000);

// Send a message with its size
let data = vec![0u8; 1000];
tx.send(data.clone(), data.len() as u64).unwrap();

// Receive the message
let received = rx.recv().unwrap();
```

## Special cases

If a message is larger than the total channel capacity, a warning is logged and the channel
waits until it's completely empty before sending (to minimize memory usage).
