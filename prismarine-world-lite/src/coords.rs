//! Defines coordinate systems and constants related to world geometry.

// Add necessary traits for bitwise operations used in coordinate calculations.
use std::ops::{BitAnd, Shr};

// --- World Geometry Constants (1.18+ including 1.21.1) ---

/// Width/Length of a chunk section in blocks (16).
pub const SECTION_WIDTH: i32 = 16;
/// Height of a chunk section in blocks (16).
pub const SECTION_HEIGHT: i32 = 16;
/// Volume of a chunk section in blocks (16*16*16 = 4096).
pub const SECTION_VOLUME: usize = (SECTION_WIDTH * SECTION_HEIGHT * SECTION_WIDTH) as usize;

/// The lowest Y level for blocks in the world (-64).
pub const MIN_CHUNK_Y: i32 = -64;
/// The total height of the world in blocks (384).
pub const WORLD_HEIGHT: i32 = 384;
/// The highest Y level for blocks in the world (320 - 1 = 319, but max block is at Y=319).
/// MAX_CHUNK_Y represents the coordinate *above* the highest block.
pub const MAX_CHUNK_Y: i32 = MIN_CHUNK_Y + WORLD_HEIGHT;

/// The number of chunk sections vertically in a chunk column (24 for 1.18+).
pub const SECTION_COUNT: usize = (WORLD_HEIGHT / SECTION_HEIGHT) as usize;
/// The Y index of the lowest chunk section (-4 for 1.18+).
pub const MIN_SECTION_Y: i32 = MIN_CHUNK_Y >> 4;
/// The Y index of the highest chunk section (19 for 1.18+). Inclusive.
pub const MAX_SECTION_Y: i32 = (MAX_CHUNK_Y >> 4) - 1;

/// Width/Length/Height of a biome section within a chunk section (4).
pub const BIOME_SECTION_DIM: i32 = 4;
/// Volume of a biome section in biome entries (4*4*4 = 64).
pub const BIOME_SECTION_VOLUME: usize =
  (BIOME_SECTION_DIM * BIOME_SECTION_DIM * BIOME_SECTION_DIM) as usize;

// --- Coordinate Structs ---

/// Represents absolute world coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoords {
  pub x: i32,
  pub y: i32,
  pub z: i32,
}

/// Represents the X and Z coordinates of a chunk column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoords {
  pub x: i32,
  pub z: i32,
}

/// Represents the coordinates of a chunk section within the world grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionCoords {
  pub x: i32,
  pub y: i32, // Section Y index (e.g., -4, 0, 19)
  pub z: i32,
}

/// Represents coordinates relative to the start (min corner) of a chunk section (0-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionRelCoords {
  pub x: i32,
  pub y: i32,
  pub z: i32,
}

/// Represents coordinates relative to the start (min corner) of a biome section (0-3).
/// Note: These coordinates are often derived from world coordinates shifted right by 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BiomeCoords {
  pub x: i32,
  pub y: i32,
  pub z: i32,
}

// --- Coordinate Conversion Implementation ---

impl WorldCoords {
  /// Converts world coordinates to the coordinates of the containing chunk column.
  pub fn to_chunk_coords(&self) -> ChunkCoords {
    ChunkCoords {
      x: self.x.shr(4), // x >> 4
      z: self.z.shr(4), // z >> 4
    }
  }

  /// Converts world coordinates to the coordinates of the containing chunk section.
  pub fn to_section_coords(&self) -> SectionCoords {
    SectionCoords {
      x: self.x.shr(4),
      y: self.y.shr(4), // Section Y index
      z: self.z.shr(4),
    }
  }

  /// Converts world coordinates to coordinates relative within their chunk section.
  pub fn to_section_rel_coords(&self) -> SectionRelCoords {
    SectionRelCoords {
      x: self.x.bitand(15),                 // x & 15
      y: self.y.rem_euclid(SECTION_HEIGHT), // y % 16 (handles negative y correctly)
      z: self.z.bitand(15),                 // z & 15
    }
  }

  /// Converts world coordinates to the coordinates used for biome indexing (4x4x4 sections).
  pub fn to_biome_coords(&self) -> BiomeCoords {
    BiomeCoords {
      x: self.x.shr(2), // x >> 2
      y: self.y.shr(2), // y >> 2
      z: self.z.shr(2), // z >> 2
    }
  }

  /// Returns the Y index of the section this coordinate is in (e.g., -4, 0, 19).
  pub fn section_y_index(&self) -> i32 {
    self.y.shr(4) // y >> 4
  }

  /// Returns the Y index of the biome section this coordinate is in.
  pub fn biome_section_y_index(&self) -> i32 {
    self.y.shr(2) // y >> 2
  }
}

// --- Indexing Helper Functions ---

/// Converts a section's Y index (e.g., -4, 0, 19) to its index in the `sections` Vec (0 to SECTION_COUNT-1).
/// Returns `None` if the Y index is outside the valid world range.
#[inline]
pub fn section_y_to_section_idx(y: i32) -> Option<usize> {
  let idx = y - MIN_SECTION_Y;
  if (0..SECTION_COUNT as i32).contains(&idx) {
    Some(idx as usize)
  } else {
    None // Y index is out of the world's bounds
  }
}

/// Converts an index in the `sections` Vec (0 to SECTION_COUNT-1) to the section's Y index (e.g., -4, 0, 19).
#[inline]
pub fn section_idx_to_section_y(idx: usize) -> i32 {
  idx as i32 + MIN_SECTION_Y
}

/// Calculates the index within a section's flat block state array (0-4095) from section-relative coordinates.
/// Assumes input coordinates are already validated (0-15).
#[inline]
pub fn get_section_block_index(coords: SectionRelCoords) -> usize {
  (coords.y as usize * SECTION_WIDTH as usize * SECTION_WIDTH as usize)
    + (coords.z as usize * SECTION_WIDTH as usize)
    + coords.x as usize
}

/// Calculates the index within a section's flat biome array (0-63) from biome coordinates.
/// Biome coordinates are relative to the start of the biome section (4x4x4).
#[inline]
pub fn get_biome_index(coords: BiomeCoords) -> usize {
  let rel_x = coords.x.rem_euclid(BIOME_SECTION_DIM);
  let rel_y = coords.y.rem_euclid(BIOME_SECTION_DIM);
  let rel_z = coords.z.rem_euclid(BIOME_SECTION_DIM);
  (rel_y as usize * BIOME_SECTION_DIM as usize * BIOME_SECTION_DIM as usize)
    + (rel_z as usize * BIOME_SECTION_DIM as usize)
    + rel_x as usize
}
