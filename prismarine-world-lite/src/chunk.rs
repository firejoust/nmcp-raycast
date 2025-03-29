//! Defines the `ChunkSection` and `ChunkColumn` structs representing Minecraft world data.

use crate::coords::{
  get_biome_index, get_section_block_index, section_y_to_section_idx, BiomeCoords,
  SectionRelCoords, WorldCoords, SECTION_COUNT,
};
use crate::palette::PaletteContainer;
use std::ops::Shr; // For the '>>' operator used in section_y_to_vec_index

/// Represents a 16x16x16 section of a chunk column.
#[derive(Debug, Clone)]
pub struct ChunkSection {
  /// Stores block state IDs using a palette system.
  block_states: PaletteContainer,
  /// Stores biome IDs using a palette system (4x4x4 resolution).
  biomes: PaletteContainer,
  /// Cached count of non-air blocks in this section. Used for rendering optimization.
  solid_block_count: i16,
}

impl ChunkSection {
  /// Creates a new chunk section.
  pub fn new(
    block_states: PaletteContainer,
    biomes: PaletteContainer,
    solid_block_count: i16,
  ) -> Self {
    ChunkSection {
      block_states,
      biomes,
      solid_block_count,
    }
  }

  /// Gets the global block state ID at the given section-relative coordinates.
  #[inline]
  pub fn get_block_state_id(&self, coords: SectionRelCoords) -> u32 {
    let index = get_section_block_index(coords);
    self.block_states.get(index)
  }

  /// Sets the global block state ID at the given section-relative coordinates.
  /// Handles palette upgrades (Single -> Indirect -> Direct) if necessary.
  pub fn set_block_state_id(
    &mut self,
    coords: SectionRelCoords,
    state_id: u32,
    global_bits_per_block: usize,
  ) {
    let index = get_section_block_index(coords);
    let old_state_id = self.block_states.get(index);

    // --- Update Solid Block Count ---
    // This is a basic check assuming state ID 0 is air. A more robust implementation
    // would use block properties from minecraft-data.
    let old_is_solid = old_state_id != 0;
    let new_is_solid = state_id != 0;

    if old_is_solid && !new_is_solid {
      self.solid_block_count -= 1;
    } else if !old_is_solid && new_is_solid {
      self.solid_block_count += 1;
    }
    // --- End Solid Block Count Update ---

    // Set the state ID in the container, potentially changing its type.
    // Pass the global bit size needed for direct palette upgrades.
    const MAX_BITS_PER_BLOCK_INDIRECT: usize = 8; // As defined in parsing.rs
    self.block_states.set(
      index,
      state_id,
      global_bits_per_block,
      MAX_BITS_PER_BLOCK_INDIRECT,
      crate::coords::SECTION_VOLUME,
    );
  }

  /// Gets the global biome ID at the given biome coordinates (4x4x4 resolution).
  #[inline]
  pub fn get_biome_id(&self, coords: BiomeCoords) -> u32 {
    let index = get_biome_index(coords);
    self.biomes.get(index)
  }

  /// Sets the global biome ID at the given biome coordinates.
  /// Handles palette upgrades if necessary.
  pub fn set_biome_id(&mut self, coords: BiomeCoords, biome_id: u32) {
    let index = get_biome_index(coords);
    // Placeholder: Assume biome palette uses 6 global bits if upgrading to direct.
    // This should ideally come from registry data.
    const GLOBAL_BITS_PER_BIOME_PLACEHOLDER: usize = 6;
    const MAX_BITS_PER_BIOME_INDIRECT: usize = 3; // As defined in parsing.rs
    self.biomes.set(
      index,
      biome_id,
      GLOBAL_BITS_PER_BIOME_PLACEHOLDER,
      MAX_BITS_PER_BIOME_INDIRECT,
      crate::coords::BIOME_SECTION_VOLUME,
    );
  }
}

/// Represents a 16x16 chunk column spanning the entire world height.
#[derive(Debug, Clone)]
pub struct ChunkColumn {
  /// Vector storing `ChunkSection`s. Indexed from 0 (lowest section) to SECTION_COUNT - 1.
  /// Use `section_y_to_section_idx` to convert world section Y to this index.
  pub sections: Vec<Option<ChunkSection>>,
  // Optional: Store block entities if needed
  // block_entities: HashMap<WorldCoords, fastnbt::Value>,
}

impl ChunkColumn {
  /// Creates a new, empty chunk column.
  pub fn new() -> Self {
    ChunkColumn {
      // Initialize with None for all possible sections according to world height.
      sections: vec![None; SECTION_COUNT],
      // block_entities: HashMap::new(),
    }
  }

  /// Helper to get the index into the `sections` Vec from a world Y coordinate.
  #[inline]
  fn world_y_to_vec_index(world_y: i32) -> Option<usize> {
    section_y_to_section_idx(world_y.shr(4)) // world_y >> 4 gives the section Y index
  }

  /// Gets a mutable reference to the chunk section at the given section Y index (e.g., -4, 0, 19).
  pub fn get_section_mut(&mut self, section_y_index: i32) -> Option<&mut ChunkSection> {
    section_y_to_section_idx(section_y_index)
      .and_then(move |idx| self.sections.get_mut(idx).and_then(|opt| opt.as_mut()))
  }

  /// Gets an immutable reference to the chunk section at the given section Y index.
  pub fn get_section(&self, section_y_index: i32) -> Option<&ChunkSection> {
    section_y_to_section_idx(section_y_index)
      .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref()))
  }

  /// Inserts or replaces a chunk section at the given section Y index.
  pub fn insert_section(&mut self, section_y_index: i32, section: ChunkSection) {
    if let Some(idx) = section_y_to_section_idx(section_y_index) {
      if idx < self.sections.len() {
        self.sections[idx] = Some(section);
      } else {
        // This should not happen if SECTION_COUNT is correct.
        eprintln!(
          "Warning: Attempted to insert section at invalid index {}",
          idx
        );
      }
    } else {
      // Section Y index is outside the world's valid range.
      eprintln!(
        "Warning: Attempted to insert section at invalid Y index {}",
        section_y_index
      );
    }
  }

  /// Gets the global block state ID at the given world coordinates. Returns 0 (air) if the section is missing.
  #[inline]
  pub fn get_block_state_id(&self, coords: WorldCoords) -> u32 {
    Self::world_y_to_vec_index(coords.y)
      .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref())) // Safely get option and then reference
      .map(|s| s.get_block_state_id(coords.to_section_rel_coords()))
      .unwrap_or(0) // Default to air (state ID 0) if section doesn't exist
  }

  /// Sets the global block state ID at the given world coordinates. Creates a new section if necessary.
  pub fn set_block_state_id(
    &mut self,
    coords: WorldCoords,
    state_id: u32,
    global_bits_per_block: usize,
  ) {
    if let Some(idx) = Self::world_y_to_vec_index(coords.y) {
      // Ensure the index is valid for the sections vector.
      if idx < self.sections.len() {
        // If the section doesn't exist and we're setting a non-air block, create it.
        if self.sections[idx].is_none() && state_id != 0 {
          // Create a new section, starting with everything as air (Single value 0).
          let mut new_section = ChunkSection::new(
            PaletteContainer::new_single(0),
            PaletteContainer::new_single(0), // Default biome 0
            0,                               // Starts with 0 solid blocks
          );
          // Now set the actual block state ID. This might upgrade the palette.
          new_section.set_block_state_id(
            coords.to_section_rel_coords(),
            state_id,
            global_bits_per_block,
          );
          self.sections[idx] = Some(new_section);
        } else if let Some(section) = self.sections[idx].as_mut() {
          // Section exists, just set the block state ID.
          section.set_block_state_id(
            coords.to_section_rel_coords(),
            state_id,
            global_bits_per_block,
          );
        }
        // If state_id is 0 (air) and the section exists, set_block_state_id handles it.
        // If state_id is 0 and the section doesn't exist, we don't need to do anything.
      }
      // Else: Y coordinate resulted in an index out of bounds (shouldn't happen with valid coords).
    }
    // Else: Y coordinate is outside the valid world range.
  }

  /// Gets the global biome ID at the given world coordinates. Returns 0 if the section is missing.
  #[inline]
  pub fn get_biome_id(&self, coords: WorldCoords) -> u32 {
    Self::world_y_to_vec_index(coords.y)
      .and_then(|idx| self.sections.get(idx).and_then(|opt| opt.as_ref()))
      .map(|s| s.get_biome_id(coords.to_biome_coords()))
      .unwrap_or(0) // Default biome 0
  }

  /// Sets the global biome ID at the given world coordinates. Creates a new section if necessary.
  pub fn set_biome_id(&mut self, coords: WorldCoords, biome_id: u32) {
    if let Some(idx) = Self::world_y_to_vec_index(coords.y) {
      if idx < self.sections.len() {
        // Create section if it doesn't exist and the biome ID is not the default (0).
        if self.sections[idx].is_none() && biome_id != 0 {
          self.sections[idx] = Some(ChunkSection::new(
            PaletteContainer::new_single(0), // Default air blocks
            PaletteContainer::new_single(0), // Default biome 0
            0,
          ));
        }
        // Set the biome if the section exists (or was just created).
        if let Some(section) = self.sections[idx].as_mut() {
          section.set_biome_id(coords.to_biome_coords(), biome_id);
        }
      }
      // Else: Y coordinate resulted in an index out of bounds.
    }
    // Else: Y coordinate is outside the valid world range.
  }

  // --- Lite Block Access ---

  /// Gets the block type ID (simplified, returns state ID in this lite version).
  #[inline]
  pub fn get_block_type_id(&self, coords: WorldCoords) -> u32 {
    self.get_block_state_id(coords)
  }

  /// Gets the block light level (placeholder).
  #[inline]
  pub fn get_block_light(&self, _coords: WorldCoords) -> u8 {
    // Placeholder - Light data parsing not implemented. Assume full light.
    15
  }

  /// Gets the sky light level (placeholder).
  #[inline]
  pub fn get_sky_light(&self, _coords: WorldCoords) -> u8 {
    // Placeholder - Light data parsing not implemented. Assume full light.
    15
  }
}
