// src/palette.rs
use byteorder::{BigEndian, ReadBytesExt};
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
        let values_per_long = BITS_PER_LONG / bits_per_value;
        let num_longs = (capacity + values_per_long - 1) / values_per_long;
        BitArray {
            data: vec![0; num_longs],
            bits_per_value,
            values_per_long,
            capacity,
            value_mask: (1u64 << bits_per_value) - 1,
        }
    }

    pub fn from_data(bits_per_value: usize, capacity: usize, data: Vec<u64>) -> Self {
         assert!(bits_per_value > 0 && bits_per_value <= 64, "bits_per_value must be between 1 and 64");
        let values_per_long = BITS_PER_LONG / bits_per_value;
        let num_longs = (capacity + values_per_long - 1) / values_per_long;
        assert!(data.len() == num_longs, "Data length mismatch");
        BitArray {
            data,
            bits_per_value,
            values_per_long,
            capacity,
            value_mask: (1u64 << bits_per_value) - 1,
        }
    }

    pub fn get(&self, index: usize) -> u32 {
        assert!(index < self.capacity, "Index out of bounds");
        let bit_index = index * self.bits_per_value;
        let long_index = bit_index / BITS_PER_LONG;
        let index_in_long = bit_index % BITS_PER_LONG;

        let mut result = self.data[long_index] >> index_in_long;

        let end_bit_offset = index_in_long + self.bits_per_value;
        if end_bit_offset > BITS_PER_LONG {
            // Value stretches across two longs
            if long_index + 1 < self.data.len() {
                 result |= self.data[long_index + 1] << (BITS_PER_LONG - index_in_long);
            }
        }

        (result & self.value_mask) as u32
    }

    pub fn set(&mut self, index: usize, value: u32) {
        assert!(index < self.capacity, "Index out of bounds");
        assert!((value as u64) <= self.value_mask, "Value does not fit into bits_per_value");

        let bit_index = index * self.bits_per_value;
        let long_index = bit_index / BITS_PER_LONG;
        let index_in_long = bit_index % BITS_PER_LONG;

        // Clear bits first
        self.data[long_index] &= !(self.value_mask << index_in_long);
        // Set new value
        self.data[long_index] |= (value as u64) << index_in_long;

        let end_bit_offset = index_in_long + self.bits_per_value;
        if end_bit_offset > BITS_PER_LONG {
            // Value stretches across two longs
            if long_index + 1 < self.data.len() {
                // Clear bits in the next long
                self.data[long_index + 1] &= !((1u64 << (end_bit_offset - BITS_PER_LONG)) - 1);
                // Set new value in the next long
                self.data[long_index + 1] |= (value as u64) >> (BITS_PER_LONG - index_in_long);
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

#[derive(Debug, Clone)]
pub enum PaletteContainer {
    Single(u32),
    Indirect { palette: Vec<u32>, data: BitArray },
    Direct(BitArray),
}

impl PaletteContainer {
    pub fn new_single(value: u32) -> Self {
        PaletteContainer::Single(value)
    }

    pub fn new_indirect(palette: Vec<u32>, data: BitArray) -> Self {
        PaletteContainer::Indirect { palette, data }
    }

     pub fn new_direct(data: BitArray) -> Self {
        PaletteContainer::Direct(data)
    }

    pub fn get(&self, index: usize) -> u32 {
        match self {
            PaletteContainer::Single(value) => *value,
            PaletteContainer::Indirect { palette, data } => {
                let palette_index = data.get(index) as usize;
                *palette.get(palette_index).unwrap_or(&0) // Default to 0 (air) if index is somehow invalid
            }
            PaletteContainer::Direct(data) => data.get(index),
        }
    }

    pub fn set(&mut self, index: usize, state_id: u32) -> bool {
        // Returns true if the container type changed (e.g., single -> indirect)
        match self {
            PaletteContainer::Single(current_value) => {
                if *current_value == state_id {
                    return false; // No change needed
                }
                // Upgrade to Indirect
                let bits = 4.max(needed_bits(1)); // Start with 4 bits or enough for 2 entries
                let mut data = BitArray::new(bits, SECTION_WIDTH as usize * SECTION_WIDTH as usize * SECTION_WIDTH as usize);
                let palette = vec![*current_value, state_id];
                // Fill existing values
                for i in 0..data.capacity {
                    if i != index {
                        data.set(i, 0); // Index of the old value
                    }
                }
                data.set(index, 1); // Index of the new value
                *self = PaletteContainer::Indirect { palette, data };
                true
            }
            PaletteContainer::Indirect { palette, data } => {
                if let Some(palette_index) = palette.iter().position(|&id| id == state_id) {
                    // State ID already in palette
                    data.set(index, palette_index as u32);
                } else {
                    // Add state ID to palette
                    let new_palette_index = palette.len();
                    palette.push(state_id);

                    let required_bits = needed_bits(new_palette_index);

                    if required_bits > data.get_bits_per_value() {
                        if required_bits <= 8 { // Max bits for section palette
                            // Resize BitArray
                            let mut new_data = BitArray::new(required_bits, data.capacity);
                            for i in 0..data.capacity {
                                new_data.set(i, data.get(i));
                            }
                             new_data.set(index, new_palette_index as u32);
                            *data = new_data;
                        } else {
                            // Upgrade to Direct
                            let global_bits = needed_bits(registry_max_state_id()); // Need a way to get this
                            let mut new_data = BitArray::new(global_bits, data.capacity);
                            for i in 0..data.capacity {
                                new_data.set(i, palette[data.get(i) as usize]);
                            }
                            new_data.set(index, state_id);
                            *self = PaletteContainer::Direct(new_data);
                            return true;
                        }
                    } else {
                         data.set(index, new_palette_index as u32);
                    }
                }
                 false
            }
            PaletteContainer::Direct(data) => {
                data.set(index, state_id);
                false
            }
        }
    }
}

// Helper to determine bits needed for a value
pub fn needed_bits(value: usize) -> usize {
    if value == 0 {
        1
    } else {
        (usize::BITS - value.leading_zeros()) as usize
    }
}

// Placeholder - replace with actual max state ID from minecraft-data for 1.21.1
fn registry_max_state_id() -> usize {
    25000 // Example value, needs to be accurate
}

// Helper function to read VarInt (implementation needed)
pub fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<i32, std::io::Error> {
    let mut num_read = 0;
    let mut result = 0;
    let mut shift = 0;
    loop {
        let byte = cursor.read_u8()?;
        num_read += 1;
        let value = (byte & 0b0111_1111) as i32;
        result |= value << shift;
        shift += 7;
        if byte & 0b1000_0000 == 0 {
            break;
        }
        if num_read > 5 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "VarInt too long"));
        }
    }
    Ok(result)
}

// Helper function to read Long Array (implementation needed)
pub fn read_long_array(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u64>, std::io::Error> {
    let len = read_varint(cursor)? as usize;
    let mut longs = Vec::with_capacity(len);
    for _ in 0..len {
        longs.push(cursor.read_u64::<BigEndian>()?);
    }
    Ok(longs)
}