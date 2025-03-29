//! The main N-API exposed struct for interacting with the world data.

use crate::chunk::ChunkColumn;
use crate::coords::{
  section_y_to_section_idx, ChunkCoords, SectionRelCoords, WorldCoords, MIN_SECTION_Y,
  SECTION_COUNT, SECTION_HEIGHT, SECTION_VOLUME, SECTION_WIDTH,
};
use crate::parsing::parse_chunk_section;
use byteorder::{LittleEndian, WriteBytesExt};
use dashmap::DashMap;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::io::Cursor;
use std::sync::{Arc, RwLock};

/// N-API accessible struct representing chunk coordinates (X, Z).
#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct JsChunkCoords {
  pub x: i32,
  pub z: i32,
}

/// The main world class exposed to Node.js via N-API.
/// Manages chunk columns and provides methods for block/biome access.
#[napi(js_name = "World")]
pub struct NapiWorld {
  /// Thread-safe hash map storing chunk columns, keyed by their coordinates.
  columns: Arc<DashMap<ChunkCoords, Arc<RwLock<ChunkColumn>>>>,
  /// The number of bits required for the global block state palette for this world's version.
  max_bits_per_block: usize,
  /// The number of bits required for the global biome palette for this world's version.
  max_bits_per_biome: usize,
}

#[napi]
impl NapiWorld {
  /// Creates a new, empty world instance.
  #[napi(constructor)]
  pub fn new() -> Self {
    // --- Version Specific Data ---
    // These should ideally be determined based on the Minecraft version
    // passed to the world instance (if that were implemented).
    // Hardcoded for 1.18+ (including 1.21.1) for now.
    const MAX_BITS_PER_BLOCK_1_21: usize = 15;
    const MAX_BITS_PER_BIOME_1_21: usize = 6;
    // --- End Version Specific Data ---

    NapiWorld {
      columns: Arc::new(DashMap::new()),
      max_bits_per_block: MAX_BITS_PER_BLOCK_1_21,
      max_bits_per_biome: MAX_BITS_PER_BIOME_1_21,
    }
  }

  /// Loads chunk column data from a network buffer (like the `map_chunk` packet data).
  /// Parses block states and biomes for all sections present in the buffer.
  ///
  /// # Arguments
  /// * `chunk_x`, `chunk_z`: The coordinates of the chunk column to load.
  /// * `data_buffer`: A Node.js `Buffer` containing the serialized chunk data.
  #[napi]
  pub fn load_column(&self, chunk_x: i32, chunk_z: i32, data_buffer: Buffer) -> Result<()> {
    let coords = ChunkCoords {
      x: chunk_x,
      z: chunk_z,
    };
    let mut cursor = Cursor::new(data_buffer.as_ref());
    let mut column = ChunkColumn::new();

    // Parse sections according to 1.18+ format (data contains sections sequentially)
    for i in 0..SECTION_COUNT {
      let section_y = MIN_SECTION_Y + i as i32;
      // Check if there's still data left in the buffer to parse a section
      if cursor.position() < cursor.get_ref().len() as u64 {
        match parse_chunk_section(
          &mut cursor,
          self.max_bits_per_block,
          self.max_bits_per_biome,
        ) {
          Ok(section) => {
            column.insert_section(section_y, section);
          }
          Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            // This can happen if the buffer ends prematurely, which might be
            // an error or just indicate fewer sections than SECTION_COUNT.
            // Log a warning and stop parsing sections for this column.
            eprintln!(
              "Warning: Unexpected EOF while parsing section y={} for chunk ({}, {}): {}",
              section_y, chunk_x, chunk_z, e
            );
            break; // Stop processing sections for this column
          }
          Err(e) => {
            // A more specific parsing error occurred.
            eprintln!(
              "Error parsing section y={} for chunk ({}, {}): {}",
              section_y, chunk_x, chunk_z, e
            );
            return Err(napi::Error::new(
              napi::Status::GenericFailure,
              format!("Failed to parse chunk section at y={}: {}", section_y, e),
            ));
          }
        }
      } else {
        // Reached the end of the buffer before iterating through all potential sections.
        // This is expected if the chunk column is not full height.
        break;
      }
    }

    // Insert the fully parsed or partially parsed column into the map.
    self.columns.insert(coords, Arc::new(RwLock::new(column)));
    Ok(())
  }

  /// Unloads a chunk column from memory.
  #[napi]
  pub fn unload_column(&self, chunk_x: i32, chunk_z: i32) {
    let coords = ChunkCoords {
      x: chunk_x,
      z: chunk_z,
    };
    self.columns.remove(&coords);
    // Consider emitting an event here if needed in JS: self.emit("chunkColumnUnload", ...)
  }

  /// Gets the global state ID of the block at the given world coordinates.
  /// Returns 0 (air) if the chunk or section is not loaded or if a lock fails.
  #[napi]
  pub fn get_block_state_id(&self, x: i32, y: i32, z: i32) -> u32 {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    self
      .columns
      .get(&chunk_coords)
      .map(|entry| {
        entry
          .value()
          .try_read() // Attempt to get read lock
          .map(|guard| guard.get_block_state_id(coords)) // If locked, get state ID
          .unwrap_or(0) // If lock fails, return 0
      })
      .unwrap_or(0) // If chunk not found, return 0
  }

  /// Sets the global state ID of the block at the given world coordinates.
  /// Creates the chunk section if it doesn't exist and `state_id` is not 0.
  /// Returns an error if the chunk is not loaded or the write lock cannot be acquired.
  #[napi]
  pub fn set_block_state_id(&self, x: i32, y: i32, z: i32, state_id: u32) -> Result<()> {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    // Use `get_mut` for potential modification
    match self.columns.get_mut(&chunk_coords) {
      Some(mut entry) => {
        // Attempt to get write lock
        match entry.value_mut().try_write() {
          Ok(mut guard) => {
            // Pass max_bits_per_block for potential palette upgrades
            guard.set_block_state_id(coords, state_id, self.max_bits_per_block);
            Ok(()) // Success
          }
          Err(_) => Err(napi::Error::new(
            napi::Status::GenericFailure,
            format!(
              "Failed to acquire write lock for chunk ({}, {})",
              chunk_coords.x, chunk_coords.z
            ),
          )),
        }
      }
      None => Err(napi::Error::new(
        // Chunk not loaded
        napi::Status::GenericFailure,
        format!("Chunk at {}, {} not loaded", chunk_coords.x, chunk_coords.z),
      )),
    }
  }

  /// Gets a simplified block object containing state ID, light levels, and biome ID.
  /// Returns `None` (-> `null` in JS) if the chunk is not loaded.
  /// Returns default values (air, max light) if the lock fails.
  #[napi]
  pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<BlockInfo> {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    self.columns.get(&chunk_coords).map(|entry| {
      match entry.value().try_read() {
        Ok(column) => BlockInfo {
          state_id: column.get_block_state_id(coords),
          light: column.get_block_light(coords),
          sky_light: column.get_sky_light(coords),
          biome_id: column.get_biome_id(coords),
        },
        // Return default block info if lock acquisition fails
        Err(_) => BlockInfo {
          state_id: 0,   // Air
          light: 15,     // Assume full block light
          sky_light: 15, // Assume full sky light
          biome_id: 0,   // Default biome
        },
      }
    })
    // If chunk is not loaded, map returns None -> null in JS
  }

  /// Gets the block light level at the given world coordinates.
  /// Returns 0 if the chunk is not loaded or if a lock fails.
  #[napi]
  pub fn get_block_light(&self, x: i32, y: i32, z: i32) -> u8 {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    self
      .columns
      .get(&chunk_coords)
      .map(|entry| match entry.value().try_read() {
        Ok(guard) => guard.get_block_light(coords),
        Err(_) => 0, // Default light 0 if lock fails
      })
      .unwrap_or(0) // Default light 0 if chunk not loaded
  }

  /// Gets the sky light level at the given world coordinates.
  /// Returns 15 (max) if the chunk is not loaded or if a lock fails.
  #[napi]
  pub fn get_sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    self
      .columns
      .get(&chunk_coords)
      .map(|entry| match entry.value().try_read() {
        Ok(guard) => guard.get_sky_light(coords),
        Err(_) => 15, // Default sky light 15 if lock fails
      })
      .unwrap_or(15) // Default sky light 15 if chunk not loaded
  }

  /// Gets the global biome ID at the given world coordinates.
  /// Returns 0 if the chunk is not loaded or if a lock fails.
  #[napi]
  pub fn get_biome_id(&self, x: i32, y: i32, z: i32) -> u32 {
    let coords = WorldCoords { x, y, z };
    let chunk_coords = coords.to_chunk_coords();
    self
      .columns
      .get(&chunk_coords)
      .map(|entry| match entry.value().try_read() {
        Ok(guard) => guard.get_biome_id(coords),
        Err(_) => 0, // Default biome 0 if lock fails
      })
      .unwrap_or(0) // Default biome 0 if chunk not loaded
  }

  /// Exports the block state IDs for a single chunk section as a Node.js `Buffer`.
  /// Returns `null` if the chunk or section is not loaded, or if the read lock fails.
  #[napi(ts_return_type = "Buffer | null")]
  pub fn export_section_states(
    &self,
    chunk_x: i32,
    chunk_z: i32,
    section_y: i32, // Section Y index (e.g., -4 to 19 for 1.18+)
  ) -> Option<Buffer> {
    let chunk_coords = ChunkCoords {
      x: chunk_x,
      z: chunk_z,
    };

    self.columns.get(&chunk_coords).and_then(|entry| {
      match entry.value().try_read() {
        // Attempt read lock
        Ok(column) => {
          // Convert section Y index to vector index
          section_y_to_section_idx(section_y).and_then(|vec_idx| {
            // Safely get the Option<ChunkSection> and then the &ChunkSection
            column.sections.get(vec_idx).and_then(|opt_section| {
              opt_section.as_ref().map(|section| {
                // Allocate buffer for 4096 u32 values (4 bytes each)
                let mut buffer_data = Vec::with_capacity(SECTION_VOLUME * 4);

                // Iterate through section coords (Y, Z, X for memory locality)
                for y_rel in 0..SECTION_HEIGHT {
                  for z_rel in 0..SECTION_WIDTH {
                    for x_rel in 0..SECTION_WIDTH {
                      let coords = SectionRelCoords {
                        x: x_rel,
                        y: y_rel,
                        z: z_rel,
                      };
                      let state_id = section.get_block_state_id(coords);
                      // Write state ID as Little Endian u32
                      buffer_data.write_u32::<LittleEndian>(state_id).unwrap(); // Panic on write error
                    }
                  }
                }
                buffer_data.into() // Convert Vec<u8> to napi::bindgen_prelude::Buffer
              })
            })
          })
        }
        Err(_) => None, // Return None if lock acquisition fails
      }
    })
    // Returns None if chunk is not loaded
  }

  /// Returns a list of coordinates for all currently loaded chunks.
  #[napi(ts_return_type = "{ x: number; z: number; }[]")]
  pub fn get_loaded_chunks(&self) -> Vec<JsChunkCoords> {
    self
      .columns
      .iter() // Iterate over the DashMap entries
      .map(|entry| JsChunkCoords {
        // Map each entry to the JS coordinate struct
        x: entry.key().x,
        z: entry.key().z,
      })
      .collect() // Collect into a Vec
  }
}

/// Simple struct to return basic block information to Node.js.
#[napi(object)]
pub struct BlockInfo {
  /// The global block state ID.
  pub state_id: u32,
  /// The block light level (0-15). Placeholder, currently always 15.
  pub light: u8,
  /// The sky light level (0-15). Placeholder, currently always 15.
  pub sky_light: u8,
  /// The global biome ID.
  pub biome_id: u32,
}
