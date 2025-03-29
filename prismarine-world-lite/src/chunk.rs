// src/chunk.rs
use crate::coords::{get_biome_index, get_section_block_index, BiomeCoords, SectionRelCoords, WorldCoords, SECTION_COUNT};
use crate::palette::PaletteContainer;
use std::collections::HashMap;
// Add the Shr trait for the >> operator
use std::ops::Shr;
// Optional: For block entities
// use fastnbt::Value;

#[derive(Debug, Clone)]
pub struct ChunkSection {
    block_states: PaletteContainer,
    biomes: PaletteContainer,
    // Optional: Add light data if needed
    // sky_light: Option<BitArray>,
    // block_light: Option<BitArray>,
    solid_block_count: i16,
}

impl ChunkSection {
    pub fn new(block_states: PaletteContainer, biomes: PaletteContainer, solid_block_count: i16) -> Self {
        ChunkSection {
            block_states,
            biomes,
            solid_block_count,
        }
    }

    pub fn get_block_state_id(&self, coords: SectionRelCoords) -> u32 {
        let index = get_section_block_index(coords);
        self.block_states.get(index)
    }

    pub fn set_block_state_id(&mut self, coords: SectionRelCoords, state_id: u32) {
        let index = get_section_block_index(coords);
        let old_state_id = self.block_states.get(index);

        // Check if the block state actually changes solid count
        // A more accurate check would involve minecraft-data, but this is a basic check
        let old_is_solid = old_state_id != 0; // Assuming 0 is air
        let new_is_solid = state_id != 0;

        if old_is_solid && !new_is_solid {
            self.solid_block_count -= 1;
        } else if !old_is_solid && new_is_solid {
            self.solid_block_count += 1;
        }

        self.block_states.set(index, state_id);
        // Note: Palette resizing/type change happens within PaletteContainer::set
    }

    pub fn get_biome_id(&self, coords: BiomeCoords) -> u32 {
        let index = get_biome_index(coords);
        self.biomes.get(index)
    }

     pub fn set_biome_id(&mut self, coords: BiomeCoords, biome_id: u32) {
        let index = get_biome_index(coords);
        self.biomes.set(index, biome_id);
         // Note: Palette resizing/type change happens within PaletteContainer::set
    }
}

#[derive(Debug, Clone)]
pub struct ChunkColumn {
    // Sections are stored by their Y index relative to MIN_SECTION_Y (0 to SECTION_COUNT-1)
    sections: Vec<Option<ChunkSection>>,
    // Optional: Store block entities if needed
    // block_entities: HashMap<WorldCoords, Value>,
}

impl ChunkColumn {
    pub fn new() -> Self {
        ChunkColumn {
            // Initialize with None for all possible sections
            sections: vec![None; SECTION_COUNT],
            // block_entities: HashMap::new(),
        }
    }

    // Helper to get the index into the `sections` Vec from a world Y coordinate
    // Use `shr` (>>) which requires `std::ops::Shr`
    fn section_y_to_vec_index(world_y: i32) -> Option<usize> {
        crate::coords::section_y_to_section_idx(world_y.shr(4))
    }

    pub fn get_section_mut(&mut self, section_y_index: i32) -> Option<&mut ChunkSection> {
         Self::section_y_to_vec_index(section_y_index << 4) // Convert section Y back to world Y for index calc
            .and_then(move |idx| self.sections[idx].as_mut())
    }

     pub fn get_section(&self, section_y_index: i32) -> Option<&ChunkSection> {
        Self::section_y_to_vec_index(section_y_index << 4) // Convert section Y back to world Y for index calc
            .and_then(|idx| self.sections[idx].as_ref())
    }

    pub fn insert_section(&mut self, section_y_index: i32, section: ChunkSection) {
        if let Some(idx) = Self::section_y_to_vec_index(section_y_index << 4) { // Convert section Y back to world Y for index calc
             if idx < self.sections.len() {
                self.sections[idx] = Some(section);
             }
        }
    }

    pub fn get_block_state_id(&self, coords: WorldCoords) -> u32 {
        Self::section_y_to_vec_index(coords.y)
            .and_then(|idx| self.sections[idx].as_ref())
            .map(|s| s.get_block_state_id(coords.to_section_rel_coords()))
            .unwrap_or(0) // Default to air if section doesn't exist
    }

    pub fn set_block_state_id(&mut self, coords: WorldCoords, state_id: u32) {
        if let Some(idx) = Self::section_y_to_vec_index(coords.y) {
            if idx < self.sections.len() {
                if self.sections[idx].is_none() && state_id != 0 {
                     // Need to create a new section if setting a non-air block
                    let mut new_section = ChunkSection::new(
                        PaletteContainer::new_single(0), // Start with air
                        PaletteContainer::new_single(0), // Default biome 0
                        0
                    );
                    new_section.set_block_state_id(coords.to_section_rel_coords(), state_id);
                    self.sections[idx] = Some(new_section);
                } else if let Some(section) = self.sections[idx].as_mut() {
                     section.set_block_state_id(coords.to_section_rel_coords(), state_id);
                }
            }
        }
    }

     pub fn get_biome_id(&self, coords: WorldCoords) -> u32 {
        Self::section_y_to_vec_index(coords.y)
             .and_then(|idx| self.sections[idx].as_ref())
            .map(|s| s.get_biome_id(coords.to_biome_coords()))
            .unwrap_or(0) // Default biome 0
    }

     pub fn set_biome_id(&mut self, coords: WorldCoords, biome_id: u32) {
         if let Some(idx) = Self::section_y_to_vec_index(coords.y) {
             if idx < self.sections.len() {
                if let Some(section) = self.sections[idx].as_mut() {
                    section.set_biome_id(coords.to_biome_coords(), biome_id);
                }
                // Optionally create section if it doesn't exist to set biome?
                // else if biome_id != 0 { ... create section ... }
             }
        }
    }

    // --- Lite Block Access ---
    // These might be simplified further depending on what the JS side needs.
    // For now, they just wrap the state/biome ID functions.

    pub fn get_block_type_id(&self, coords: WorldCoords) -> u32 {
        // In a real scenario, this would map state_id to type_id via minecraft-data
        // For a lite version, just returning state_id might be sufficient.
        self.get_block_state_id(coords)
    }

    pub fn get_block_light(&self, _coords: WorldCoords) -> u8 {
        // Placeholder - Light data parsing not implemented
        15
    }

     pub fn get_sky_light(&self, _coords: WorldCoords) -> u8 {
        // Placeholder - Light data parsing not implemented
        15
    }
}