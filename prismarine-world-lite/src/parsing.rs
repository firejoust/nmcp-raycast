// src/parsing.rs
use crate::palette::{BitArray, PaletteContainer, read_long_array, read_varint};
use crate::chunk::ChunkSection;
use crate::coords::{SECTION_HEIGHT, SECTION_WIDTH};
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

// Constants remain the same...
const GLOBAL_BITS_PER_BLOCK: usize = 15;
const MAX_BITS_PER_BLOCK: usize = 8;
const MIN_BITS_PER_BLOCK: usize = 4;
const BIOME_SECTION_VOLUME: usize = 4 * 4 * 4;
const GLOBAL_BITS_PER_BIOME: usize = 6;
const MAX_BITS_PER_BIOME: usize = 3;
const MIN_BITS_PER_BIOME: usize = 1;


pub fn parse_chunk_section(cursor: &mut Cursor<&[u8]>, section_y: i32) -> Result<ChunkSection, std::io::Error> {
    let start_pos = cursor.position();
    eprintln!("\n--- Parsing Section y={} ---", section_y);
    eprintln!("[parse_section y={}] Start cursor: {}", section_y, start_pos);

    let solid_block_count = cursor.read_i16::<BigEndian>()?;
    eprintln!("[parse_section y={}] Solid block count: {} (Cursor after: {})", section_y, solid_block_count, cursor.position());

    let block_states_container = parse_palette_container(
        cursor,
        (SECTION_WIDTH * SECTION_WIDTH * SECTION_HEIGHT) as usize,
        MIN_BITS_PER_BLOCK,
        MAX_BITS_PER_BLOCK,
        GLOBAL_BITS_PER_BLOCK,
        &format!("Blocks (y={})", section_y)
    )?;
    let cursor_after_blocks = cursor.position();
    eprintln!("[parse_section y={}] Cursor after blocks: {}", section_y, cursor_after_blocks);

    let biomes_container = parse_palette_container(
        cursor,
        BIOME_SECTION_VOLUME,
        MIN_BITS_PER_BIOME,
        MAX_BITS_PER_BIOME,
        GLOBAL_BITS_PER_BIOME,
        &format!("Biomes (y={})", section_y)
    )?;
    let cursor_after_biomes = cursor.position();
    eprintln!("[parse_section y={}] Cursor after biomes: {}", section_y, cursor_after_biomes);
    eprintln!("[parse_section y={}] Total bytes read for section: {}", section_y, cursor_after_biomes - start_pos);

    Ok(ChunkSection::new(block_states_container, biomes_container, solid_block_count))
}

fn parse_palette_container(
    cursor: &mut Cursor<&[u8]>,
    capacity: usize,
    _min_bits: usize,
    max_bits_indirect: usize,
    bits_direct: usize,
    context: &str,
) -> Result<PaletteContainer, std::io::Error> {
    let cursor_before_bits = cursor.position();
    let bits_per_value = cursor.read_u8()? as usize;
    eprintln!("[parse_palette {}] Cursor before bits: {}, Bits per value: {} (Cursor after: {})", context, cursor_before_bits, bits_per_value, cursor.position());

    if bits_per_value == 0 {
        // Single value palette
        let cursor_before_val = cursor.position();
        let value = read_varint(cursor)? as u32;
        let cursor_after_val = cursor.position();
        let cursor_before_len = cursor.position();
        let data_array_len_ignored = read_varint(cursor)?;
        eprintln!("[parse_palette {}] Type: Single. Cursor before val: {}, Value: {} (Cursor after: {}). Cursor before len: {}, Discarded DataLen: {} (Cursor after: {})",
            context, cursor_before_val, value, cursor_after_val, cursor_before_len, data_array_len_ignored, cursor.position());
        if data_array_len_ignored != 0 {
             eprintln!("[parse_palette {}] WARNING: Single value palette had non-zero data array length: {}", context, data_array_len_ignored);
        }
        Ok(PaletteContainer::new_single(value))

    } else if bits_per_value <= max_bits_indirect {
        // Indirect (section palette)
        let cursor_before_pal_len = cursor.position();
        let palette_len = read_varint(cursor)? as usize;
        eprintln!("[parse_palette {}] Type: Indirect. Cursor before pal_len: {}, Palette length: {} (Cursor after: {})", context, cursor_before_pal_len, palette_len, cursor.position());

        if palette_len == 0 {
             eprintln!("[parse_palette {}] WARNING: Indirect palette has zero length!", context);
             let cursor_before_len = cursor.position();
             let data_long_len = read_varint(cursor)? as usize;
             eprintln!("[parse_palette {}] Cursor before data len: {}, Expected Data array length (VarInt): {} (Cursor after: {})", context, cursor_before_len, data_long_len, cursor.position());
             let cursor_before_data = cursor.position();
             let _data_longs = read_long_array(cursor, data_long_len)?;
             eprintln!("[parse_palette {}] Cursor before data read: {}, Read {} longs (Cursor after: {})", context, cursor_before_data, data_long_len, cursor.position());
             return Ok(PaletteContainer::new_indirect(vec![], BitArray::new(bits_per_value, capacity)));
        }

        let cursor_before_palette = cursor.position();
        let mut palette = Vec::with_capacity(palette_len);
        for _ in 0..palette_len {
            palette.push(read_varint(cursor)? as u32);
        }
        eprintln!("[parse_palette {}] Cursor before palette: {}, Read Palette: {:?} (Cursor after: {})", context, cursor_before_palette, palette, cursor.position());

        let cursor_before_len = cursor.position();
        let data_long_len = read_varint(cursor)? as usize;
        eprintln!("[parse_palette {}] Cursor before data len: {}, Expected Data array length (VarInt): {} (Cursor after: {})", context, cursor_before_len, data_long_len, cursor.position());
        let cursor_before_data = cursor.position();
        let data_longs = read_long_array(cursor, data_long_len)?;
        eprintln!("[parse_palette {}] Cursor before data read: {}, Actual Data longs read: {} (Cursor after: {})", context, cursor_before_data, data_longs.len(), cursor.position());
        // eprintln!("[parse_palette {}] Data longs (first 5): {:?}", context, data_longs.iter().take(5).map(|&x| format!("{:#x}", x)).collect::<Vec<_>>());

        let bit_array = BitArray::from_data(bits_per_value, capacity, data_longs);
        Ok(PaletteContainer::new_indirect(palette, bit_array))
    } else {
        // Direct (global palette)
        let cursor_before_len = cursor.position();
        let data_long_len = read_varint(cursor)? as usize;
        eprintln!("[parse_palette {}] Type: Direct. Cursor before data len: {}, Expected Data array length (VarInt): {} (Cursor after: {})", context, cursor_before_len, data_long_len, cursor.position());
        let cursor_before_data = cursor.position();
        let data_longs = read_long_array(cursor, data_long_len)?;
        eprintln!("[parse_palette {}] Cursor before data read: {}, Actual Data longs read: {} (Cursor after: {})", context, cursor_before_data, data_longs.len(), cursor.position());
        // eprintln!("[parse_palette {}] Data longs (first 5): {:?}", context, data_longs.iter().take(5).map(|&x| format!("{:#x}", x)).collect::<Vec<_>>());

        let effective_bits = bits_direct.max(bits_per_value);
        eprintln!("[parse_palette {}] Effective bits for Direct: {}", context, effective_bits);
        let bit_array = BitArray::from_data(effective_bits, capacity, data_longs);
        Ok(PaletteContainer::new_direct(bit_array))
    }
}