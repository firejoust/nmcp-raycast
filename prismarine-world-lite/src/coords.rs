// src/coords.rs
// Add Shr and BitAnd traits for the >> and & operators
use std::ops::{BitAnd, Shr};

pub const SECTION_WIDTH: i32 = 16;
pub const SECTION_HEIGHT: i32 = 16;
// For 1.18+ (including 1.21.1)
pub const MIN_CHUNK_Y: i32 = -64;
pub const WORLD_HEIGHT: i32 = 384;
pub const MAX_CHUNK_Y: i32 = MIN_CHUNK_Y + WORLD_HEIGHT;
pub const SECTION_COUNT: usize = (WORLD_HEIGHT / SECTION_HEIGHT) as usize;
pub const MIN_SECTION_Y: i32 = MIN_CHUNK_Y >> 4;
pub const MAX_SECTION_Y: i32 = (MAX_CHUNK_Y >> 4) -1; // Inclusive max section index

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChunkCoords {
    pub x: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionRelCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BiomeCoords {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl WorldCoords {
    pub fn to_chunk_coords(&self) -> ChunkCoords {
        ChunkCoords {
            x: self.x.shr(4), // x >> 4
            z: self.z.shr(4), // z >> 4
        }
    }

    pub fn to_section_coords(&self) -> SectionCoords {
        SectionCoords {
            x: self.x.shr(4),
            y: self.y.shr(4),
            z: self.z.shr(4),
        }
    }

    pub fn to_section_rel_coords(&self) -> SectionRelCoords {
        SectionRelCoords {
            x: self.x.bitand(15), // x & 15
            y: self.y.rem_euclid(16), // Use rem_euclid for correct modulo with negative Y
            z: self.z.bitand(15), // z & 15
        }
    }

    pub fn to_biome_coords(&self) -> BiomeCoords {
        BiomeCoords {
            x: self.x.shr(2), // x >> 2
            y: self.y.shr(2), // y >> 2
            z: self.z.shr(2), // z >> 2
        }
    }

    pub fn section_y_index(&self) -> i32 {
        self.y.shr(4) // y >> 4
    }

    // This was incorrect, should use rem_euclid for section-relative Y
    // pub fn section_rel_y(&self) -> i32 {
    //     self.y.rem_euclid(16)
    // }

    pub fn biome_section_y_index(&self) -> i32 {
        self.y.shr(2) // y >> 2
    }

    // This was incorrect, should use rem_euclid for biome-relative Y
    // pub fn biome_rel_y(&self) -> i32 {
    //     self.y.rem_euclid(4)
    // }
}

pub fn section_idx_to_section_y(idx: usize) -> i32 {
    idx as i32 + MIN_SECTION_Y
}

pub fn section_y_to_section_idx(y: i32) -> Option<usize> {
    let idx = y - MIN_SECTION_Y;
    if idx >= 0 && idx < SECTION_COUNT as i32 {
        Some(idx as usize)
    } else {
        None
    }
}

/// Calculates the index within a section's flat array (4096 elements)
pub fn get_section_block_index(coords: SectionRelCoords) -> usize {
    (coords.y as usize * SECTION_WIDTH as usize * SECTION_WIDTH as usize) +
    (coords.z as usize * SECTION_WIDTH as usize) +
    coords.x as usize
}

/// Calculates the index within a biome section's flat array (64 elements)
pub fn get_biome_index(coords: BiomeCoords) -> usize {
    let rel_x = coords.x.rem_euclid(4);
    let rel_y = coords.y.rem_euclid(4);
    let rel_z = coords.z.rem_euclid(4);
    (rel_y as usize * 4 * 4) + (rel_z as usize * 4) + rel_x as usize
}