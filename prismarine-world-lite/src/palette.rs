//! Implements compact storage using palettes and bit arrays, similar to Minecraft's chunk format.

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Write};

const BITS_PER_LONG: usize = 64;

/// A packed array of unsigned integers.
/// Stores values using a fixed number of bits per value.
/// Adheres to the Minecraft 1.16+ format where values do NOT span across u64 boundaries.
#[derive(Debug, Clone)]
pub struct BitArray {
  /// The underlying packed data.
  data: Vec<u64>,
  /// Number of bits used for each value.
  bits_per_value: usize,
  /// Maximum number of values this array can hold.
  capacity: usize,
  /// Bitmask to extract a single value (e.g., 0b1111 for 4 bits).
  value_mask: u64,
}

impl BitArray {
  /// Creates a new BitArray filled with zeros.
  ///
  /// # Panics
  /// Panics if `bits_per_value` is greater than 64.
  pub fn new(bits_per_value: usize, capacity: usize) -> Self {
    assert!(
      bits_per_value <= BITS_PER_LONG,
      "bits_per_value cannot exceed 64"
    );

    let num_longs = if bits_per_value == 0 {
      0 // No storage needed if values have 0 bits
    } else {
      let values_per_long = BITS_PER_LONG / bits_per_value;
      // Calculate longs needed based on capacity, ensuring no value spans longs
      (capacity + values_per_long - 1) / values_per_long
    };

    BitArray {
      data: vec![0; num_longs],
      bits_per_value,
      capacity,
      value_mask: if bits_per_value == BITS_PER_LONG {
        u64::MAX
      } else {
        (1u64 << bits_per_value) - 1
      },
    }
  }

  /// Creates a BitArray from existing packed u64 data.
  ///
  /// # Panics
  /// Panics if `bits_per_value` > 64 or if the provided `data` length doesn't match
  /// the calculated required length based on `bits_per_value` and `capacity`.
  pub fn from_data(bits_per_value: usize, capacity: usize, data: Vec<u64>) -> Self {
    assert!(
      bits_per_value <= BITS_PER_LONG,
      "bits_per_value cannot exceed 64"
    );

    let num_longs = if bits_per_value == 0 {
      0
    } else {
      let values_per_long = BITS_PER_LONG / bits_per_value;
      (capacity + values_per_long - 1) / values_per_long
    };

    // Allow empty data only if capacity or bits_per_value is 0
    if !(data.len() == num_longs
      || (capacity == 0 && data.is_empty())
      || (bits_per_value == 0 && data.is_empty()))
    {
      panic!(
        "Data length mismatch: expected {}, got {}",
        num_longs,
        data.len()
      );
    }

    BitArray {
      data,
      bits_per_value,
      capacity,
      value_mask: if bits_per_value == BITS_PER_LONG {
        u64::MAX
      } else {
        (1u64 << bits_per_value) - 1
      },
    }
  }

  /// Gets the value at the given index.
  ///
  /// # Panics
  /// Panics if `index` is out of bounds (`>= capacity`).
  #[inline]
  pub fn get(&self, index: usize) -> u32 {
    assert!(
      index < self.capacity,
      "Index out of bounds: {} >= {}",
      index,
      self.capacity
    );
    if self.bits_per_value == 0 {
      return 0;
    } // All values are 0 if 0 bits are used

    let values_per_long = BITS_PER_LONG / self.bits_per_value;
    let long_index = index / values_per_long;
    let index_in_long = (index % values_per_long) * self.bits_per_value;

    // Defensive check, though assert should cover this
    if long_index >= self.data.len() {
      return 0;
    }

    // Extract the value from the correct long at the calculated bit offset
    let result = (self.data[long_index] >> index_in_long) & self.value_mask;
    result as u32
  }

  /// Sets the value at the given index.
  ///
  /// # Panics
  /// Panics if `index` is out of bounds, or if `value` requires more bits than `bits_per_value`.
  #[inline]
  pub fn set(&mut self, index: usize, value: u32) {
    assert!(
      index < self.capacity,
      "Index out of bounds: {} >= {}",
      index,
      self.capacity
    );
    assert!(
      (value as u64) <= self.value_mask,
      "Value {} does not fit into {} bits",
      value,
      self.bits_per_value
    );
    if self.bits_per_value == 0 {
      return;
    } // Cannot set values if 0 bits are used

    let values_per_long = BITS_PER_LONG / self.bits_per_value;
    let long_index = index / values_per_long;
    let index_in_long = (index % values_per_long) * self.bits_per_value;

    // Defensive check
    if long_index >= self.data.len() {
      panic!("long_index out of bounds during set");
    }

    // Clear the bits where the value will be placed
    self.data[long_index] &= !(self.value_mask << index_in_long);
    // Set the new value at the correct bit offset
    self.data[long_index] |= (value as u64) << index_in_long;
  }

  /// Returns a reference to the underlying u64 data vector.
  pub fn get_data(&self) -> &Vec<u64> {
    &self.data
  }

  /// Returns the number of bits used per value.
  pub fn get_bits_per_value(&self) -> usize {
    self.bits_per_value
  }

  /// Returns the number of u64 values in the underlying data vector.
  pub fn len(&self) -> usize {
    self.data.len()
  }

  /// Writes the packed u64 data to a writer in Big Endian format.
  pub fn write_to_buffer<W: Write>(&self, writer: &mut W) -> Result<(), std::io::Error> {
    for &long_val in &self.data {
      writer.write_u64::<BigEndian>(long_val)?;
    }
    Ok(())
  }
}

/// Represents the different ways data (like block states or biomes) can be stored in a chunk section.
#[derive(Debug, Clone)]
pub enum PaletteContainer {
  /// All entries in the container have the same single value.
  Single(u32),
  /// Entries are indices into a local `palette` list. Uses a `BitArray` (`data`) to store indices.
  Indirect { palette: Vec<u32>, data: BitArray },
  /// Entries are direct global registry IDs. Uses a `BitArray` (`data`) to store IDs.
  Direct(BitArray),
}

impl PaletteContainer {
  /// Creates a new Single value container.
  pub fn new_single(value: u32) -> Self {
    PaletteContainer::Single(value)
  }

  /// Creates a new Indirect container with the given palette and data.
  pub fn new_indirect(palette: Vec<u32>, data: BitArray) -> Self {
    PaletteContainer::Indirect { palette, data }
  }

  /// Creates a new Direct container with the given data.
  pub fn new_direct(data: BitArray) -> Self {
    PaletteContainer::Direct(data)
  }

  /// Gets the global registry ID at the given index within the container's capacity (e.g., 0-4095 for blocks).
  #[inline]
  pub fn get(&self, index: usize) -> u32 {
    match self {
      PaletteContainer::Single(value) => *value,
      PaletteContainer::Indirect { palette, data } => {
        let palette_index = data.get(index) as usize;
        // Return the value from the palette, defaulting to 0 if the index is somehow invalid.
        *palette.get(palette_index).unwrap_or(&0)
      }
      PaletteContainer::Direct(data) => data.get(index),
    }
  }

  /// Sets the global registry ID at the given index.
  /// May cause the container type to change (e.g., Single -> Indirect, Indirect -> Direct).
  /// Returns `true` if the container type changed, `false` otherwise.
  ///
  /// # Arguments
  /// * `index` - The index within the container's capacity (e.g., 0-4095 for blocks).
  /// * `state_id` - The global registry ID to set.
  /// * `global_bits_per_value` - The number of bits required for the global palette (used when upgrading to Direct).
  /// * `max_indirect_bits` - The maximum number of bits allowed for an Indirect palette before upgrading to Direct.
  /// * `capacity` - The total capacity of this container (e.g., 4096 for blocks, 64 for biomes).
  pub fn set(
    &mut self,
    index: usize,
    state_id: u32,
    global_bits_per_value: usize,
    max_indirect_bits: usize,
    capacity: usize,
  ) -> bool {
    match self {
      PaletteContainer::Single(current_value) => {
        if *current_value == state_id {
          return false; // Value is already set, no change needed.
        }
        // --- Upgrade from Single to Indirect ---
        // Start with 4 bits (or more if needed for 2 entries, though unlikely).
        let initial_bits = 4.max(needed_bits(1));
        let mut data = BitArray::new(initial_bits, capacity);
        let palette = vec![*current_value, state_id];

        // Initialize the new BitArray. All existing slots implicitly had the old value (index 0).
        // We only need to explicitly set the *new* value at the target index (index 1).
        // Note: A full loop is technically correct but less efficient.
        // for i in 0..capacity { data.set(i, 0); } // Set all to old value index
        data.set(index, 1); // Set the target index to the new value index

        *self = PaletteContainer::Indirect { palette, data };
        true // Container type changed
      }
      PaletteContainer::Indirect { palette, data } => {
        // Check if the state ID is already in the palette.
        if let Some(palette_index) = palette.iter().position(|&id| id == state_id) {
          // Yes, just update the data array.
          data.set(index, palette_index as u32);
          false // Container type did not change
        } else {
          // No, add the state ID to the palette.
          let new_palette_index = palette.len();
          palette.push(state_id);
          let required_bits = needed_bits(new_palette_index); // Bits needed for the new palette size

          if required_bits > data.get_bits_per_value() {
            // Palette grew, need more bits per value in the data array.
            if required_bits <= max_indirect_bits {
              // --- Resize Indirect Palette ---
              let mut new_data = BitArray::new(required_bits, data.capacity);
              // Copy existing data, indices remain the same.
              for i in 0..data.capacity {
                new_data.set(i, data.get(i));
              }
              // Set the new value at the target index using the new palette index.
              new_data.set(index, new_palette_index as u32);
              *data = new_data; // Replace the old BitArray
              false // Container type did not change (still Indirect)
            } else {
              // --- Upgrade from Indirect to Direct ---
              let mut new_data = BitArray::new(global_bits_per_value, data.capacity);
              // Copy existing data, converting palette indices to global state IDs.
              for i in 0..data.capacity {
                new_data.set(i, palette[data.get(i) as usize]);
              }
              // Set the new value at the target index using the global state ID.
              new_data.set(index, state_id);
              *self = PaletteContainer::Direct(new_data); // Replace self with the new Direct container
              true // Container type changed
            }
          } else {
            // Palette didn't require resizing the BitArray, just set the index.
            data.set(index, new_palette_index as u32);
            false // Container type did not change
          }
        }
      }
      PaletteContainer::Direct(data) => {
        // Already using global IDs, just set the value.
        data.set(index, state_id);
        false // Container type did not change
      }
    }
  }
}

/// Calculates the minimum number of bits required to store the given non-negative value.
#[inline]
pub fn needed_bits(value: usize) -> usize {
  if value == 0 {
    1 // Need at least 1 bit even for value 0
  } else {
    // Equivalent to ceil(log2(value + 1))
    (usize::BITS - value.leading_zeros()) as usize
  }
}

// --- VarInt Reading ---
/// Reads a VarInt from the cursor.
pub fn read_varint(cursor: &mut Cursor<&[u8]>) -> Result<i32, std::io::Error> {
  let mut num_read = 0;
  let mut result = 0;
  let mut shift = 0;
  loop {
    let byte = cursor.read_u8()?;
    num_read += 1;
    let value = (byte & 0b0111_1111) as i32;
    // Use overflowing_shl for safety, although standard VarInts shouldn't overflow i32 here.
    result |= value.overflowing_shl(shift).0;
    shift += 7;
    if byte & 0b1000_0000 == 0 {
      break; // Stop when the most significant bit is 0
    }
    if num_read > 5 {
      // VarInts encoding i32 should not be longer than 5 bytes
      return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "VarInt too long",
      ));
    }
  }
  Ok(result)
}

// --- Long Array Reading ---
/// Reads a VarInt length-prefixed array of Big Endian u64 values from the cursor.
pub fn read_long_array(cursor: &mut Cursor<&[u8]>) -> Result<Vec<u64>, std::io::Error> {
  let len = read_varint(cursor)? as usize;
  // Sanity check: Prevent allocating huge vectors based on potentially corrupt data.
  // Check if the declared number of longs would exceed the remaining buffer length.
  let remaining_bytes = cursor.get_ref().len() - cursor.position() as usize;
  if len * 8 > remaining_bytes {
    return Err(std::io::Error::new(
      std::io::ErrorKind::InvalidData,
      format!(
        "Long array length ({}) too large for remaining buffer ({})",
        len, remaining_bytes
      ),
    ));
  }
  let mut longs = Vec::with_capacity(len);
  for _ in 0..len {
    longs.push(cursor.read_u64::<BigEndian>()?);
  }
  Ok(longs)
}
