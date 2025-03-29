// src/chunk.rs
// No changes needed from the previous version.
use crate::coords::{get_biome_index, get_section_block_index, BiomeCoords, SectionRelCoords, WorldCoords, SECTION_COUNT, MIN_SECTION_Y};
use crate::palette::PaletteContainer;
use std::ops::Shr;

#[derive(Debug, Clone)]
pub struct ChunkSection {
    block_states: PaletteContainer,
    biomes: PaletteContainer,
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

    pub fn set_block_state_id(&mut self, coords: SectionRelCoords, state_id: u32, global_bits_per_block: usize) {
        let index = get_section_block_index(coords);
        let old_state_id = self.block_states.get(index);

        let old_is_solid = old_state_id != 0;
        let new_is_solid = state_id != 0;

        if old_is_solid && !new_is_solid {
            self.solid_block_count -= 1;
        } else if !old_is_solid && new_is_solid {
            self.solid_block_count += 1;
        }

        self.block_states.set(index, state_id, global_bits_per_block);
    }

    pub fn get_biome_id(&self, coords: BiomeCoords) -> u32 {
        let index = get_biome_index(coords);
        self.biomes.get(index)
    }

     pub fn set_biome_id(&mut self, coords: BiomeCoords, biome_id: u32) {
        let index = get_biome_index(coords);
        const GLOBAL_BITS_PER_BIOME_PLACEHOLDER: usize = 6;
        self.biomes.set(index, biome_id, GLOBAL_BITS_PER_BIOME_PLACEHOLDER);
    }
}

#[derive(Debug, Clone)]
pub struct ChunkColumn {
    pub sections: Vec<Option<ChunkSection>>,
    // block_entities: HashMap<WorldCoords, Value>, // Optional
}

impl ChunkColumn {
    pub fn new() -> Self {
        ChunkColumn {
            sections: vec![None; SECTION_COUNT],
            // block_entities: HashMap::new(),
        }
    }

    fn section_y_to_vec_index(world_y: i32) -> Option<usize> {
        crate::coords::section_y_to_section_idx(world_y.shr(4))
    }

    pub fn get_section_mut(&mut self, section_y_index: i32) -> Option<&mut ChunkSection> {
         Self::section_y_to_vec_index(section_y_index << 4)
            .and_then(move |idx| self.sections.get_mut(idx).and_then(|opt| opt.as_mut()))
    }

     pub fn get_section(&self, section_y_index: i32) -> Option<&ChunkSection> {
        Self::section_y_to_vec_index(section_y_index << 4)
            .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref()))
    }

    pub fn insert_section(&mut self, section_y_index: i32, section: ChunkSection) {
        if let Some(idx) = Self::section_y_to_vec_index(section_y_index << 4) {
             if idx < self.sections.len() {
                self.sections[idx] = Some(section);
             } else {
                 eprintln!("Warning: Attempted to insert section at invalid index {}", idx);
             }
        } else {
             eprintln!("Warning: Attempted to insert section at invalid Y index {}", section_y_index);
        }
    }

    pub fn get_block_state_id(&self, coords: WorldCoords) -> u32 {
        Self::section_y_to_vec_index(coords.y)
            .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref()))
            .map(|s| s.get_block_state_id(coords.to_section_rel_coords()))
            .unwrap_or(0)
    }

    pub fn set_block_state_id(&mut self, coords: WorldCoords, state_id: u32, global_bits_per_block: usize) {
        if let Some(idx) = Self::section_y_to_vec_index(coords.y) {
            if idx < self.sections.len() {
                if self.sections[idx].is_none() && state_id != 0 {
                    let mut new_section = ChunkSection::new(
                        PaletteContainer::new_single(0),
                        PaletteContainer::new_single(0),
                        0
                    );
                    new_section.set_block_state_id(coords.to_section_rel_coords(), state_id, global_bits_per_block);
                    self.sections[idx] = Some(new_section);
                } else if let Some(section) = self.sections[idx].as_mut() {
                     section.set_block_state_id(coords.to_section_rel_coords(), state_id, global_bits_per_block);
                }
            }
        }
    }

     pub fn get_biome_id(&self, coords: WorldCoords) -> u32 {
        Self::section_y_to_vec_index(coords.y)
             .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref()))
            .map(|s| s.get_biome_id(coords.to_biome_coords()))
            .unwrap_or(0)
    }

     pub fn set_biome_id(&mut self, coords: WorldCoords, biome_id: u32) {
         if let Some(idx) = Self::section_y_to_vec_index(coords.y) {
             if idx < self.sections.len() {
                 if self.sections[idx].is_none() && biome_id != 0 {
                     self.sections[idx] = Some(ChunkSection::new(
                         PaletteContainer::new_single(0),
                         PaletteContainer::new_single(0),
                         0
                     ));
                 }
                if let Some(section) = self.sections[idx].as_mut() {
                    section.set_biome_id(coords.to_biome_coords(), biome_id);
                }
             }
        }
    }

    pub fn get_block_type_id(&self, coords: WorldCoords) -> u32 {
        self.get_block_state_id(coords)
    }

    pub fn get_block_light(&self, _coords: WorldCoords) -> u8 {
        15
    }

     pub fn get_sky_light(&self, _coords: WorldCoords) -> u8 {
        15
    }
}