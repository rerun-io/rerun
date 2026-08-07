# re_lerobot

Part of the [`rerun`](https://github.com/rerun-io/rerun) family of crates.

[![Latest version](https://img.shields.io/crates/v/re_lerobot.svg?speculative-link)](https://crates.io/crates/re_lerobot?speculative-link)
[![Documentation](https://docs.rs/re_lerobot/badge.svg?speculative-link)](https://docs.rs/re_lerobot?speculative-link)
![MIT](https://img.shields.io/badge/license-MIT-blue.svg)
![Apache](https://img.shields.io/badge/license-Apache-blue.svg)

Core LeRobot-to-chunk loading logic for Rerun.

Reads LeRobot datasets (v2 and v3) into Rerun chunks: enumerate episodes with `iter_episode_indices()`, load each episode's chunks with `load_episode_chunks()`.
