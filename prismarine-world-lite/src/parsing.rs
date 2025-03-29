// src/parsing.rs
// No changes needed here from the previous version, as the direct palette
// parsing logic already correctly uses `bits_direct`. The fix is within BitArray.
use crate::palette::{BitArray, PaletteContainer, read_long_array, read_varint};
use crate::chunk::ChunkSection;
use crate::coords::{SECTION_HEIGHT, SECTION_WIDTH, BIOME_SECTION_VOLUME};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

// --- Constants based on 1.18+ ---
const MAX_BITS_PER_BLOCK_INDIRECT: usize = 8;
const MAX_BITS_PER_BIOME_INDIRECT: usize = 3;
const MIN_BITS_PER_BLOCK: usize = 4; // Not directly used in parsing but relevant for context
const MIN_BITS_PER_BIOME: usize = 1; // Not directly used in parsing but relevant for context
// --- End Constants ---

// Placeholder: This should ideally come from a registry or config for the specific MC version
// For 1.21.1, the max block state ID requires 15 bits.
const GLOBAL_BITS_PER_BLOCK: usize = 15;
// For 1.21.1, the max biome ID requires 6 bits.
const GLOBAL_BITS_PER_BIOME: usize = 6;


pub fn parse_chunk_section(cursor: &mut Cursor<&[u8]>) -> Result<ChunkSection, std::io::Error> {
    let solid_block_count = cursor.read_i16::<BigEndian>()?;

    let block_states_container = parse_palette_container(
        cursor,
        SECTION_WIDTH as usize * SECTION_WIDTH as usize * SECTION_HEIGHT as usize,
        MAX_BITS_PER_BLOCK_INDIRECT,
        GLOBAL_BITS_PER_BLOCK,
    )?;
    let biomes_container = parse_palette_container(
        cursor,
        BIOME_SECTION_VOLUME,
        MAX_BITS_PER_BIOME_INDIRECT,
        GLOBAL_BITS_PER_BIOME,
    )?;

    Ok(ChunkSection::new(block_states_container, biomes_container, solid_block_count))
}

fn parse_palette_container(
    cursor: &mut Cursor<&[u8]>,
    capacity: usize,
    max_bits_indirect: usize,
    bits_direct: usize,
) -> Result<PaletteContainer, std::io::Error> {
    let bits_per_value_packet = cursor.read_u8()? as usize;

    if bits_per_value_packet == 0 {
        // Single value palette
        let value = read_varint(cursor)? as u32;
        let data_array_len = read_varint(cursor)?;
        if data_array_len != 0 {
             return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Non-zero data array length for single value palette"));
        }
        Ok(PaletteContainer::new_single(value))

    } else if bits_per_value_packet <= max_bits_indirect {
        // Indirect (section palette)
        let palette_len = read_varint(cursor)? as usize;
        if palette_len == 0 || palette_len > capacity {
             return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Invalid indirect palette length: {}", palette_len)));
        }
        let mut palette = Vec::with_capacity(palette_len);
        for _ in 0..palette_len {
            palette.push(read_varint(cursor)? as u32);
        }
        let data_longs = read_long_array(cursor)?;
        let bit_array = BitArray::from_data(bits_per_value_packet, capacity, data_longs);
        Ok(PaletteContainer::new_indirect(palette, bit_array))

    } else {
        // Direct (global palette)
        let data_longs = read_long_array(cursor)?;
        // Use bits_direct for the BitArray size, as bits_per_value_packet just signals it's direct.
        let bit_array = BitArray::from_data(bits_direct, capacity, data_longs);
        Ok(PaletteContainer::new_direct(bit_array))
    }
}