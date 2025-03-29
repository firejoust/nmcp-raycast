//! Functions for parsing chunk section data from network buffers.

use crate::chunk::ChunkSection;
use crate::coords::{BIOME_SECTION_VOLUME, SECTION_VOLUME};
use crate::palette::{read_long_array, read_varint, BitArray, PaletteContainer};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

// --- Palette/Container Constants (1.18+ including 1.21.1) ---
// These define the thresholds and bit sizes for block and biome palettes.

/// Maximum bits per entry for an indirect block palette before switching to direct.
const MAX_BITS_PER_BLOCK_INDIRECT: usize = 8;
/// Maximum bits per entry for an indirect biome palette before switching to direct.
const MAX_BITS_PER_BIOME_INDIRECT: usize = 3;
// Minimum bits per entry used in palettes (when not single value).
// const MIN_BITS_PER_BLOCK: usize = 4; // Not directly used in parsing
// const MIN_BITS_PER_BIOME: usize = 1; // Not directly used in parsing

/// Parses a complete chunk section (block states + biomes) from the cursor.
/// Assumes the format used since Minecraft 1.18.
pub fn parse_chunk_section(
  cursor: &mut Cursor<&[u8]>,
  global_bits_per_block: usize,
  global_bits_per_biome: usize,
) -> Result<ChunkSection, std::io::Error> {
  // Read the number of non-air blocks in the section.
  let solid_block_count = cursor.read_i16::<BigEndian>()?;

  // Parse the block states container.
  let block_states_container = parse_palette_container(
    cursor,
    SECTION_VOLUME,
    MAX_BITS_PER_BLOCK_INDIRECT,
    global_bits_per_block,
  )?;

  // Parse the biomes container.
  let biomes_container = parse_palette_container(
    cursor,
    BIOME_SECTION_VOLUME,
    MAX_BITS_PER_BIOME_INDIRECT,
    global_bits_per_biome,
  )?;

  Ok(ChunkSection::new(
    block_states_container,
    biomes_container,
    solid_block_count,
  ))
}

/// Parses a Paletted Container structure (either for blocks or biomes) from the cursor.
/// Determines the container type (Single, Indirect, Direct) based on the bits per value.
///
/// # Arguments
/// * `cursor`: The data stream.
/// * `capacity`: The total number of entries the container holds (e.g., 4096 for blocks, 64 for biomes).
/// * `max_bits_indirect`: The maximum bits per entry allowed for an Indirect palette for this container type.
/// * `bits_direct`: The number of bits per entry to use when creating a Direct palette for this container type.
fn parse_palette_container(
  cursor: &mut Cursor<&[u8]>,
  capacity: usize,
  max_bits_indirect: usize,
  bits_direct: usize,
) -> Result<PaletteContainer, std::io::Error> {
  // The first byte indicates the bits per value, determining the palette type.
  let bits_per_value_packet = cursor.read_u8()? as usize;

  match bits_per_value_packet {
    0 => {
      // --- Single Value Palette ---
      // The entire container holds only one value.
      let value = read_varint(cursor)? as u32;
      // Read and discard the data array length (it's always 0 for single value).
      let data_array_len = read_varint(cursor)?;
      if data_array_len != 0 {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          format!(
            "Non-zero data array length ({}) for single value palette",
            data_array_len
          ),
        ));
      }
      Ok(PaletteContainer::new_single(value))
    }
    bpv if bpv <= max_bits_indirect => {
      // --- Indirect Palette (Section Palette) ---
      // Values in the data array are indices into a local palette.
      let palette_len = read_varint(cursor)? as usize;
      // Basic validation for palette length.
      if palette_len == 0 || palette_len > capacity {
        return Err(std::io::Error::new(
          std::io::ErrorKind::InvalidData,
          format!("Invalid indirect palette length: {}", palette_len),
        ));
      }
      // Read the palette entries (global registry IDs).
      let mut palette = Vec::with_capacity(palette_len);
      for _ in 0..palette_len {
        palette.push(read_varint(cursor)? as u32);
      }
      // Read the packed data array (indices into the palette).
      let data_longs = read_long_array(cursor)?;
      let bit_array = BitArray::from_data(bpv, capacity, data_longs);
      Ok(PaletteContainer::new_indirect(palette, bit_array))
    }
    _ => {
      // --- Direct Palette (Global Palette) ---
      // Values in the data array are direct global registry IDs.
      // The bits_per_value read from the packet only signals that it's direct.
      // We must use the pre-determined `bits_direct` for the BitArray.
      let data_longs = read_long_array(cursor)?;
      let bit_array = BitArray::from_data(bits_direct, capacity, data_longs);
      Ok(PaletteContainer::new_direct(bit_array))
    }
  }
}
