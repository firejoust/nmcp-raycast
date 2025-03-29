// src/parsing.rs
use crate::palette::{BitArray, PaletteContainer, read_long_array, read_varint};
use crate::chunk::ChunkSection;
// Removed MIN_SECTION_Y, SECTION_COUNT from use statement
use crate::coords::{SECTION_HEIGHT, SECTION_WIDTH};
use byteorder::{BigEndian, ReadBytesExt};
// Removed unused Read trait
use std::io::Cursor;

const GLOBAL_BITS_PER_BLOCK: usize = 15; // Example for 1.18+, adjust if needed for 1.21.1
const MAX_BITS_PER_BLOCK: usize = 8; // Max for section palette before going global
const MIN_BITS_PER_BLOCK: usize = 4;

const BIOME_SECTION_VOLUME: usize = 4 * 4 * 4;
const GLOBAL_BITS_PER_BIOME: usize = 6; // Example, adjust if needed
const MAX_BITS_PER_BIOME: usize = 3;
const MIN_BITS_PER_BIOME: usize = 1;


pub fn parse_chunk_section(cursor: &mut Cursor<&[u8]>) -> Result<ChunkSection, std::io::Error> {
    let solid_block_count = cursor.read_i16::<BigEndian>()?;
    // Cast the i32 result to usize
    let block_states_container = parse_palette_container(cursor, (SECTION_WIDTH * SECTION_WIDTH * SECTION_HEIGHT) as usize, MIN_BITS_PER_BLOCK, MAX_BITS_PER_BLOCK, GLOBAL_BITS_PER_BLOCK)?;
    let biomes_container = parse_palette_container(cursor, BIOME_SECTION_VOLUME, MIN_BITS_PER_BIOME, MAX_BITS_PER_BIOME, GLOBAL_BITS_PER_BIOME)?;

    Ok(ChunkSection::new(block_states_container, biomes_container, solid_block_count))
}

fn parse_palette_container(
    cursor: &mut Cursor<&[u8]>,
    capacity: usize,
    _min_bits: usize, // Prefixed with underscore as it's unused
    max_bits_indirect: usize,
    bits_direct: usize,
) -> Result<PaletteContainer, std::io::Error> {
    let bits_per_value = cursor.read_u8()? as usize;

    if bits_per_value == 0 {
        // Single value palette
        let value = read_varint(cursor)? as u32;
        read_varint(cursor)?; // Read and discard data array length (always 0 for single value)
        Ok(PaletteContainer::new_single(value))
    } else if bits_per_value <= max_bits_indirect {
        // Indirect (section palette)
        let palette_len = read_varint(cursor)? as usize;
        let mut palette = Vec::with_capacity(palette_len);
        for _ in 0..palette_len {
            palette.push(read_varint(cursor)? as u32);
        }
        let data_longs = read_long_array(cursor)?;
        // Ensure capacity matches what the BitArray expects
        let bit_array = BitArray::from_data(bits_per_value, capacity, data_longs);
        Ok(PaletteContainer::new_indirect(palette, bit_array))
    } else {
        // Direct (global palette)
        let data_longs = read_long_array(cursor)?;
         // Ensure capacity matches what the BitArray expects
        let bit_array = BitArray::from_data(bits_direct.max(bits_per_value), capacity, data_longs); // Use max(bits_direct, bits_per_value) for direct
        Ok(PaletteContainer::new_direct(bit_array))
    }
}