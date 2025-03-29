// src/palette.rs
use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use std::io::Cursor;

use crate::coords::SECTION_WIDTH;

const BITS_PER_LONG: usize = 64;

#[derive(Debug, Clone)]
pub struct BitArray {
    data: Vec<u64>,
    bits_per_value: usize,
    values_per_long: usize,
    capacity: usize,
    value_mask: u64,
}

impl BitArray {
    pub fn new(bits_per_value: usize, capacity: usize) -> Self {
        assert!(bits_per_value > 0 && bits_per_value <= 64, "bits_per_value must be between 1 and 64");
        let values_per_long = if bits_per_value == 0 { 0 } else { BITS_PER_LONG / bits_per_value }; // Avoid division by zero
        let num_longs = if values_per_long == 0 { 0 } else { (capacity + values_per_long - 1) / values_per_long };
        BitArray {
            data: vec![0; num_longs],
            bits_per_value,
            values_per_long,
            capacity,
            value_mask: if bits_per_value == 64 { u64::MAX } else { (1u64 << bits_per_value) - 1 }, // Handle 64 bits case
        }
    }

    pub fn from_data(bits_per_value: usize, capacity: usize, data: Vec<u64>) -> Self {
        assert!(bits_per_value > 0 && bits_per_value <= 64, "bits_per_value must be between 1 and 64");
        let values_per_long = if bits_per_value == 0 { 0 } else { BITS_PER_LONG / bits_per_value };
        let num_longs = if values_per_long == 0 { 0 } else { (capacity + values_per_long - 1) / values_per_long };

        eprintln!("[BitArray::from_data] bits: {}, capacity: {}, received data len: {}", bits_per_value, capacity, data.len());
        eprintln!("[BitArray::from_data] Calculated num_longs: {}", num_longs);
        eprintln!("[BitArray::from_data] Received data (first 5): {:?}", data.iter().take(5).map(|&x| format!("{:#x}", x)).collect::<Vec<_>>());

        if data.len() != num_longs {
             eprintln!("[BitArray::from_data] ERROR: Data length mismatch! Expected {}, got {}. This is likely a parsing error.", num_longs, data.len());
             // Return a default/empty BitArray or panic, as this indicates a fundamental issue upstream.
             // Returning an empty one might hide the error source. Let's panic for debugging.
             panic!("BitArray data length mismatch during creation. Expected {}, got {}. Check parsing logic.", num_longs, data.len());
             // Or return a default:
             // return BitArray::new(bits_per_value, capacity);
        }

        BitArray {
            data,
            bits_per_value,
            values_per_long,
            capacity,
            value_mask: if bits_per_value == 64 { u64::MAX } else { (1u64 << bits_per_value) - 1 },
        }
    }

    // --- GET METHOD WITH SPECIFIC DEBUGGING ---
    pub fn get(&self, index: usize) -> u32 {
        assert!(index < self.capacity, "Index out of bounds: {} >= {}", index, self.capacity);
        let bit_index = index * self.bits_per_value;
        let long_index_start = bit_index / BITS_PER_LONG;
        let bit_index_start = bit_index % BITS_PER_LONG; // Index within the first long (0-63)

        // --- Specific Debugging for index 3814 when bits_per_value is 5 ---
        let is_target_for_debug = index == 3814 && self.bits_per_value == 5;
        if is_target_for_debug {
            eprintln!("[BitArray::get DEBUG index=3814, bits=5]");
            eprintln!("  bit_index: {}", bit_index); // Should be 19070
            eprintln!("  long_index_start: {}", long_index_start); // Should be 297
            eprintln!("  bit_index_start: {}", bit_index_start); // Should be 62
            if long_index_start < self.data.len() {
                eprintln!("  data[{}]: {:#018x}", long_index_start, self.data[long_index_start]);
            } else {
                 eprintln!("  data[{}] is out of bounds!", long_index_start);
            }
            if long_index_start + 1 < self.data.len() {
                 eprintln!("  data[{}]: {:#018x}", long_index_start + 1, self.data[long_index_start + 1]);
            } else {
                 eprintln!("  data[{}] is out of bounds!", long_index_start + 1);
            }
        }
        // --- End Specific Debugging ---

        if long_index_start >= self.data.len() {
             eprintln!("[BitArray::get] ERROR: long_index_start out of bounds ({} >= {}). Index: {}, bits_per_value: {}", long_index_start, self.data.len(), index, self.bits_per_value);
             return 0;
        }

        let current_long = self.data[long_index_start];
        let bits_remaining_in_long = BITS_PER_LONG - bit_index_start;

        let result: u64;

        if bits_remaining_in_long >= self.bits_per_value {
            // Value fits entirely within the current long
            result = (current_long >> bit_index_start) & self.value_mask;
             if is_target_for_debug { eprintln!("  Fits in one long. Intermediate result: {:#x}", result); }
        } else {
            // Value spans across two longs
            let bits_from_first = bits_remaining_in_long;
            let bits_from_second = self.bits_per_value - bits_from_first;

            // Get the lower part from the end of the first long
            let first_part = current_long >> bit_index_start;

            // Get the upper part from the start of the second long
            let second_part = if long_index_start + 1 < self.data.len() {
                let next_long = self.data[long_index_start + 1];
                // Mask to get only the needed lower bits from the next long
                let second_mask = (1u64 << bits_from_second) - 1;
                next_long & second_mask
            } else {
                 eprintln!("[BitArray::get] WARNING: Accessing index {} requires long {}, but data len is only {}. Result might be incomplete.", index, long_index_start + 1, self.data.len());
                0
            };

            // Combine the parts
            result = first_part | (second_part << bits_from_first);
            if is_target_for_debug {
                eprintln!("  Spans longs.");
                eprintln!("    bits_from_first: {}", bits_from_first); // Should be 2
                eprintln!("    bits_from_second: {}", bits_from_second); // Should be 3
                eprintln!("    first_part (shifted current_long): {:#x}", first_part);
                eprintln!("    second_part (masked next_long): {:#x}", second_part);
                eprintln!("    combined result before final mask: {:#x}", result);
            }
        }

        let final_result = (result & self.value_mask) as u32;
        if is_target_for_debug { eprintln!("  Final masked result: {} ({:#x})", final_result, final_result); }

        final_result
    }
    // --- End GET METHOD ---

    // --- Set method (ensure it matches get logic if modified) ---
    pub fn set(&mut self, index: usize, value: u32) {
        assert!(index < self.capacity, "Index out of bounds: {} >= {}", index, self.capacity);
        let value_u64 = value as u64;
        assert!(value_u64 <= self.value_mask, "Value {} does not fit into {} bits (mask {:#x})", value, self.bits_per_value, self.value_mask);

        let bit_index = index * self.bits_per_value;
        let long_index = bit_index / BITS_PER_LONG;
        let bit_index_in_long = bit_index % BITS_PER_LONG;

        if long_index >= self.data.len() {
             eprintln!("[BitArray::set] ERROR: long_index out of bounds ({} >= {}). Index: {}, Value: {}, bits_per_value: {}", long_index, self.data.len(), index, value, self.bits_per_value);
             return; // Or handle error
        }

        // --- Clear the bits where the value will be written ---
        // Mask for bits within the first long
        let first_long_mask = self.value_mask << bit_index_in_long;
        self.data[long_index] &= !first_long_mask;

        // --- Write the value parts ---
        // Write the lower part (that fits in the first long)
        self.data[long_index] |= (value_u64 & self.value_mask) << bit_index_in_long;

        // Check if it spans across longs
        let bits_in_first_long = BITS_PER_LONG - bit_index_in_long;
        if bits_in_first_long < self.bits_per_value {
            if long_index + 1 < self.data.len() {
                let bits_in_second_long = self.bits_per_value - bits_in_first_long;
                // Mask for the bits in the second long (lowest bits)
                let second_long_mask = (1u64 << bits_in_second_long) - 1;
                // Clear the bits in the second long
                self.data[long_index + 1] &= !second_long_mask;
                // Write the upper part of the value (shifted down) into the second long
                self.data[long_index + 1] |= (value_u64 >> bits_in_first_long) & second_long_mask;
            } else {
                 eprintln!("[BitArray::set] WARNING: Attempting to write index {} which spans beyond data length {}. Data might be lost.", index, self.data.len());
            }
        }
    }

     pub fn get_data(&self) -> &Vec<u64> {
        &self.data
    }

    pub fn get_bits_per_value(&self) -> usize {
        self.bits_per_value
    }

     pub fn len(&self) -> usize {
        self.data.len()
    }
}

// --- PaletteContainer (with refined logging in get) ---
#[derive(Debug, Clone)]
pub enum PaletteContainer {
    Single(u32),
    Indirect { palette: Vec<u32>, data: BitArray },
    Direct(BitArray),
}

impl PaletteContainer {
    pub fn new_single(value: u32) -> Self { PaletteContainer::Single(value) }
    pub fn new_indirect(palette: Vec<u32>, data: BitArray) -> Self { PaletteContainer::Indirect { palette, data } }
    pub fn new_direct(data: BitArray) -> Self { PaletteContainer::Direct(data) }

    pub fn get(&self, index: usize) -> u32 {
        match self {
            PaletteContainer::Single(value) => *value,
            PaletteContainer::Indirect { palette, data } => {
                let palette_index = data.get(index) as usize;
                if palette_index >= palette.len() {
                    eprintln!("[PaletteContainer::get Indirect] ERROR: Palette index {} out of bounds for palette len {}. Index: {}, BitsPerVal: {}",
                        palette_index, palette.len(), index, data.get_bits_per_value());
                    // Log more context if needed:
                    // eprintln!("Palette contents: {:?}", palette);
                    // eprintln!("BitArray data (first 5): {:?}", data.get_data().iter().take(5).map(|&x| format!("{:#x}", x)).collect::<Vec<_>>());
                    0 // Default to air
                } else {
                    palette[palette_index]
                }
            }
            PaletteContainer::Direct(data) => data.get(index),
        }
    }

    // Set method remains the same as previous version with logging
    pub fn set(&mut self, index: usize, state_id: u32) -> bool {
        match self {
            PaletteContainer::Single(current_value) => {
                if *current_value == state_id { return false; }
                let bits = 4.max(needed_bits(1));
                let capacity = (SECTION_WIDTH * SECTION_WIDTH * SECTION_WIDTH) as usize;
                let mut data = BitArray::new(bits, capacity);
                let palette = vec![*current_value, state_id];
                for i in 0..capacity { if i != index { data.set(i, 0); } }
                data.set(index, 1);
                eprintln!("[PaletteContainer::set] Upgraded Single({}) to Indirect({:?}) at index {}", current_value, palette, index);
                *self = PaletteContainer::Indirect { palette, data };
                true
            }
            PaletteContainer::Indirect { palette, data } => {
                let old_palette_index = data.get(index) as usize;
                let old_state_id = palette.get(old_palette_index).copied().unwrap_or(0);
                if old_state_id == state_id { return false; }

                if let Some(palette_index) = palette.iter().position(|&id| id == state_id) {
                    data.set(index, palette_index as u32);
                    // eprintln!("[PaletteContainer::set Indirect] Set index {} to existing palette index {}", index, palette_index);
                } else {
                    let new_palette_index = palette.len();
                    palette.push(state_id);
                    // eprintln!("[PaletteContainer::set Indirect] Added state {} to palette at index {}", state_id, new_palette_index);
                    let required_bits = needed_bits(new_palette_index);

                    if required_bits > data.get_bits_per_value() {
                        if required_bits <= 8 {
                            // eprintln!("[PaletteContainer::set Indirect] Resizing BitArray from {} to {} bits", data.get_bits_per_value(), required_bits);
                            let mut new_data = BitArray::new(required_bits, data.capacity);
                            for i in 0..data.capacity { new_data.set(i, data.get(i)); }
                            new_data.set(index, new_palette_index as u32);
                            *data = new_data;
                        } else {
                            let global_bits = needed_bits(registry_max_state_id());
                            eprintln!("[PaletteContainer::set Indirect] Upgrading to Direct ({} bits)", global_bits);
                            let mut new_data = BitArray::new(global_bits, data.capacity);
                            for i in 0..data.capacity { new_data.set(i, palette[data.get(i) as usize]); }
                            new_data.set(index, state_id);
                            *self = PaletteContainer::Direct(new_data);
                            return true;
                        }
                    } else {
                         data.set(index, new_palette_index as u32);
                         // eprintln!("[PaletteContainer::set Indirect] Set index {} to new palette index {}", index, new_palette_index);
                    }
                }
                 false
            }
            PaletteContainer::Direct(data) => {
                let old_state_id = data.get(index);
                if old_state_id == state_id { return false; }
                data.set(index, state_id);
                // eprintln!("[PaletteContainer::set Direct] Set index {} to state {}", index, state_id);
                false
            }
        }
    }
}

// --- Helper functions (needed_bits, registry_max_state_id, read_varint) remain the same ---
pub fn needed_bits(value: usize) -> usize {
    if value == 0 { 1 } else { (usize::BITS - value.leading_zeros()) as usize }
}
fn registry_max_state_id() -> usize { 26000 } // Placeholder
pub fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<i32, std::io::Error> {
    let mut num_read = 0; let mut result = 0; let mut shift = 0;
    loop {
        if cursor.position() >= cursor.get_ref().len() as u64 { return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Reached EOF while reading VarInt byte")); }
        let byte = cursor.read_u8()?; num_read += 1; let value = (byte & 0b0111_1111) as i32;
        result |= value.checked_shl(shift).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "VarInt shift overflow"))?;
        shift += 7; if byte & 0b1000_0000 == 0 { break; }
        if num_read > 5 { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "VarInt too long")); }
    } Ok(result)
}
// --- Modified read_long_array ---
pub fn read_long_array(cursor: &mut Cursor<&[u8]>, expected_len: usize) -> Result<Vec<u64>, std::io::Error> {
    let mut longs = Vec::with_capacity(expected_len);
    for i in 0..expected_len {
        if cursor.position() + 8 > cursor.get_ref().len() as u64 {
             eprintln!("[read_long_array] ERROR: Unexpected EOF. Expected long {}, but cursor is at {} and buffer len is {}", i, cursor.position(), cursor.get_ref().len());
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, format!("Expected long {}/{}, but reached EOF", i, expected_len)));
        }
        // --- TRY READING AS LITTLE ENDIAN ---
        longs.push(cursor.read_u64::<LittleEndian>()?);
        // --- END ---
    } Ok(longs)
}