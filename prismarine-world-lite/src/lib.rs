#![deny(clippy::all)]

// Removed #[macro_use] as it wasn't needed for napi_derive
extern crate napi_derive;

mod chunk;
mod coords;
mod palette;
mod parsing;
mod world;

// No functions needed at the top level for this example,
// everything is exposed via the NapiWorld class.