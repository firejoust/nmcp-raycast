// src/palette.rs
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Write};

use crate::coords::SECTION_WIDTH;

const BITS_PER_LONG: usize = 64;

#[derive(Debug, Clone)]
pub struct BitArray {
    data: Vec<u64>,
    bits_per_value: usize,
    // values_per_long: usize, // This becomes less relevant with no spanning
    capacity: usize,
    value_mask: u64,
}

impl BitArray {
    pub fn new(bits_per_value: usize, capacity: usize) -> Self {
        assert!(bits_per_value <= 64, "bits_per_value cannot exceed 64");
        // Calculate longs needed based on capacity and bits_per_value,
        // considering that values don't span across longs.
        let num_longs = if bits_per_value == 0 {
            0
        } else {
            let values_per_long = BITS_PER_LONG / bits_per_value;
            (capacity + values_per_long - 1) / values_per_long
        };

        BitArray {
            data: vec![0; num_longs],
            bits_per_value,
            capacity,
            value_mask: if bits_per_value == 64 { u64::MAX } else { (1u64 << bits_per_value) - 1 },
        }
    }

    pub fn from_data(bits_per_value: usize, capacity: usize, data: Vec<u64>) -> Self {
        assert!(bits_per_value <= 64, "bits_per_value cannot exceed 64");
        // Recalculate expected longs based on no-spanning rule
        let num_longs = if bits_per_value == 0 {
            0
        } else {
            let values_per_long = BITS_PER_LONG / bits_per_value;
            (capacity + values_per_long - 1) / values_per_long
        };

        if !(data.len() == num_longs || (capacity == 0 && data.is_empty()) || (bits_per_value == 0 && data.is_empty())) {
             panic!("Data length mismatch: expected {}, got {}", num_longs, data.len());
        }
        BitArray {
            data,
            bits_per_value,
            capacity,
            value_mask: if bits_per_value == 64 { u64::MAX } else { (1u64 << bits_per_value) - 1 },
        }
    }

    // Corrected get for 1.16+ (no spanning longs)
    pub fn get(&self, index: usize) -> u32 {
        assert!(index < self.capacity, "Index out of bounds");
        if self.bits_per_value == 0 { return 0; }

        let values_per_long = BITS_PER_LONG / self.bits_per_value;
        let long_index = index / values_per_long;
        let index_in_long = (index % values_per_long) * self.bits_per_value;

        if long_index >= self.data.len() {
            // Should not happen with correct capacity/data length, but handle defensively
            return 0;
        }

        let result = (self.data[long_index] >> index_in_long) & self.value_mask;
        result as u32
    }

    // Corrected set for 1.16+ (no spanning longs)
    pub fn set(&mut self, index: usize, value: u32) {
        assert!(index < self.capacity, "Index out of bounds");
        assert!((value as u64) <= self.value_mask, "Value does not fit into bits_per_value");
        if self.bits_per_value == 0 { return; }

        let values_per_long = BITS_PER_LONG / self.bits_per_value;
        let long_index = index / values_per_long;
        let index_in_long = (index % values_per_long) * self.bits_per_value;

        if long_index >= self.data.len() {
            panic!("long_index out of bounds during set");
        }

        // Clear bits first
        self.data[long_index] &= !(self.value_mask << index_in_long);
        // Set new value
        self.data[long_index] |= (value as u64) << index_in_long;
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

    pub fn write_to_buffer<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
        for &long_val in &self.data {
            writer.write_u64::<BigEndian>(long_val)?;
        }
        Ok(())
    }
}

// --- PaletteContainer and other functions remain the same as the previous corrected version ---
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
                *palette.get(palette_index).unwrap_or(&0)
            }
            PaletteContainer::Direct(data) => data.get(index),
        }
    }

    pub fn set(&mut self, index: usize, state_id: u32, global_bits_per_value: usize) -> bool {
        match self {
            PaletteContainer::Single(current_value) => {
                if *current_value == state_id {
                    return false;
                }
                let bits = 4.max(needed_bits(1));
                let capacity = SECTION_WIDTH as usize * SECTION_WIDTH as usize * SECTION_WIDTH as usize;
                let mut data = BitArray::new(bits, capacity);
                let palette = vec![*current_value, state_id];
                for i in 0..capacity {
                    if i != index { data.set(i, 0); }
                }
                data.set(index, 1);
                *self = PaletteContainer::Indirect { palette, data };
                true
            }
            PaletteContainer::Indirect { palette, data } => {
                if let Some(palette_index) = palette.iter().position(|&id| id == state_id) {
                    data.set(index, palette_index as u32);
                    false
                } else {
                    let new_palette_index = palette.len();
                    palette.push(state_id);
                    let required_bits = needed_bits(new_palette_index);

                    if required_bits > data.get_bits_per_value() {
                        if required_bits <= 8 { // Max bits for section palette
                            let mut new_data = BitArray::new(required_bits, data.capacity);
                            for i in 0..data.capacity { new_data.set(i, data.get(i)); }
                            new_data.set(index, new_palette_index as u32);
                            *data = new_data;
                            false
                        } else { // Upgrade to Direct
                            let mut new_data = BitArray::new(global_bits_per_value, data.capacity);
                            for i in 0..data.capacity { new_data.set(i, palette[data.get(i) as usize]); }
                            new_data.set(index, state_id);
                            *self = PaletteContainer::Direct(new_data);
                            true
                        }
                    } else {
                         data.set(index, new_palette_index as u32);
                         false
                    }
                }
            }
            PaletteContainer::Direct(data) => {
                data.set(index, state_id);
                false
            }
        }
    }
}

pub fn needed_bits(value: usize) -> usize {
    if value == 0 { 1 } else { (usize::BITS - value.leading_zeros()) as usize }
}

pub fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<i32, std::io::Error> {
    let mut num_read = 0;
    let mut result = 0;
    let mut shift = 0;
    loop {
        let byte = cursor.read_u8()?;
        num_read += 1;
        let value = (byte & 0b0111_1111) as i32;
        result |= value.overflowing_shl(shift).0;
        shift += 7;
        if byte & 0b1000_0000 == 0 { break; }
        if num_read > 5 { return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "VarInt too long")); }
    }
    Ok(result)
}

pub fn read_long_array(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u64>, std::io::Error> {
    let len = read_varint(cursor)? as usize;
    if len > (cursor.get_ref().len() / 8) + 1 {
         return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Long array length too large for buffer"));
    }
    let mut longs = Vec::with_capacity(len);
    for _ in 0..len { longs.push(cursor.read_u64::<BigEndian>()?); }
    Ok(longs)
}