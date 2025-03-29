#![deny(clippy::all)]

//! A lightweight NAPI-RS implementation for managing Minecraft world data (chunk columns).
//! Provides basic block and biome access optimized for performance.

// Make modules public so `NapiWorld` can use items from them.
pub mod chunk;
pub mod coords;
pub mod palette;
pub mod parsing;
pub mod world;
