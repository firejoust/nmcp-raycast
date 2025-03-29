// src/world.rs
use crate::chunk::ChunkColumn;
use crate::coords::{ChunkCoords, WorldCoords, section_y_to_section_idx, MIN_SECTION_Y, SECTION_COUNT}; // Added section_y_to_section_idx
use crate::parsing::parse_chunk_section;
use napi::bindgen_prelude::*;
use napi_derive::napi;
// Removed unused HashMap
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use dashmap::DashMap; // Use DashMap for concurrent access

#[napi(js_name = "World")]
pub struct NapiWorld {
    // Use DashMap for thread-safe interior mutability needed by NAPI methods
    // Arc<RwLock<ChunkColumn>> allows shared ownership and read/write locking per chunk
    columns: Arc<DashMap<ChunkCoords, Arc<RwLock<ChunkColumn>>>>,
}

#[napi]
impl NapiWorld {
    #[napi(constructor)]
    pub fn new() -> Self {
        NapiWorld {
            columns: Arc::new(DashMap::new()),
        }
    }

    /// Loads chunk column data from a network buffer (like `map_chunk` packet data).
    #[napi]
    pub fn load_column(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        data_buffer: Buffer,
        // bit_map: Buffer, // TODO: Handle bitmap for partial updates if needed
    ) -> Result<()> {
        let coords = ChunkCoords { x: chunk_x, z: chunk_z };
        let mut cursor = Cursor::new(data_buffer.as_ref());
        let mut column = ChunkColumn::new();

        // Parse sections according to 1.18+ format (data contains sections sequentially)
        for i in 0..SECTION_COUNT {
             let section_y = MIN_SECTION_Y + i as i32;
             // Check if cursor has enough data before attempting to parse
             if cursor.position() < cursor.get_ref().len() as u64 {
                match parse_chunk_section(&mut cursor) {
                    Ok(section) => {
                        column.insert_section(section_y, section);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        // Reached end of buffer, likely fewer sections than max were sent
                        break;
                    }
                    Err(e) => {
                        // Log the error for debugging, but don't necessarily stop loading other sections
                        eprintln!("Error parsing section y={} for chunk ({}, {}): {}", section_y, chunk_x, chunk_z, e);
                        // Optionally break or continue based on desired strictness
                        // break;
                        // Or return Err(...) if any section failure is critical
                         return Err(napi::Error::new(
                             napi::Status::GenericFailure,
                             format!("Failed to parse chunk section at y={}: {}", section_y, e),
                         ));
                    }
                }
             } else {
                 break; // No more data in buffer
             }
        }

        // TODO: Parse block entities if needed, they come after sections

        // Store the loaded column using Arc<RwLock<>>
        self.columns.insert(coords, Arc::new(RwLock::new(column)));

        // TODO: Emit chunkColumnLoad event via NAPI if needed (requires ThreadSafeFunction)

        Ok(())
    }

    /// Unloads a chunk column.
    #[napi]
    pub fn unload_column(&self, chunk_x: i32, chunk_z: i32) {
        let coords = ChunkCoords { x: chunk_x, z: chunk_z };
        self.columns.remove(&coords);
        // TODO: Emit chunkColumnUnload event via NAPI if needed
    }

    /// Gets the state ID of the block at the given world coordinates.
    #[napi]
    pub fn get_block_state_id(&self, x: i32, y: i32, z: i32) -> u32 {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();

        // Use a read lock to access the column
        self.columns
            .get(&chunk_coords)
            .map(|entry| {
                // Use try_read to avoid blocking if a write lock is held briefly
                match entry.value().try_read() {
                    Ok(guard) => guard.get_block_state_id(coords),
                    Err(_) => 0 // Or handle contention differently
                }
            })
            .unwrap_or(0) // Default to air if chunk not loaded
    }

    /// Sets the state ID of the block at the given world coordinates.
    #[napi]
    pub fn set_block_state_id(&self, x: i32, y: i32, z: i32, state_id: u32) -> Result<()> {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();

        // Use a write lock to modify the column
        match self.columns.get_mut(&chunk_coords) {
            Some(mut entry) => {
                 // Use try_write to avoid blocking if a read lock is held briefly
                match entry.value_mut().try_write() {
                    Ok(mut guard) => {
                        guard.set_block_state_id(coords, state_id);
                         // TODO: Emit blockUpdate event via NAPI if needed
                        Ok(())
                    },
                    Err(_) => Err(napi::Error::new(
                        napi::Status::GenericFailure,
                        "Failed to acquire write lock for chunk".to_string(),
                    ))
                }
            }
            None => Err(napi::Error::new(
                napi::Status::GenericFailure,
                format!("Chunk at {}, {} not loaded", chunk_coords.x, chunk_coords.z),
            )),
        }
    }

    // --- Lite Block Access ---

    /// Gets a simplified block object (stateId, light, skyLight, biomeId).
    #[napi]
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> Option<BlockInfo> {
         let coords = WorldCoords { x, y, z };
         let chunk_coords = coords.to_chunk_coords();

         self.columns.get(&chunk_coords).map(|entry| {
             // Use try_read for potentially better performance in read-heavy scenarios
             match entry.value().try_read() {
                 Ok(column) => BlockInfo {
                     state_id: column.get_block_state_id(coords),
                     light: column.get_block_light(coords),
                     sky_light: column.get_sky_light(coords),
                     biome_id: column.get_biome_id(coords),
                 },
                 Err(_) => BlockInfo { // Return default/air if lock contended
                     state_id: 0,
                     light: 0,
                     sky_light: 15,
                     biome_id: 0,
                 }
             }
         })
    }

     /// Gets the block light level at the given world coordinates.
    #[napi]
    pub fn get_block_light(&self, x: i32, y: i32, z: i32) -> u8 {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();
        self.columns
            .get(&chunk_coords)
            .map(|entry| match entry.value().try_read() {
                Ok(guard) => guard.get_block_light(coords),
                Err(_) => 0
            })
            .unwrap_or(0)
    }

    /// Gets the sky light level at the given world coordinates.
    #[napi]
    pub fn get_sky_light(&self, x: i32, y: i32, z: i32) -> u8 {
         let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();
        self.columns
            .get(&chunk_coords)
            .map(|entry| match entry.value().try_read() {
                Ok(guard) => guard.get_sky_light(coords),
                Err(_) => 15
            })
            .unwrap_or(15) // Default to full sky light if chunk not loaded
    }

    /// Gets the biome ID at the given world coordinates.
    #[napi]
    pub fn get_biome_id(&self, x: i32, y: i32, z: i32) -> u32 {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();
        self.columns
            .get(&chunk_coords)
            .map(|entry| match entry.value().try_read() {
                Ok(guard) => guard.get_biome_id(coords),
                Err(_) => 0
            })
            .unwrap_or(0) // Default biome 0
    }

    // --- Setters for light/biome (Optional for lite version) ---

    // #[napi]
    // pub fn set_block_light(&self, x: i32, y: i32, z: i32, light: u8) -> Result<()> { ... }
    // #[napi]
    // pub fn set_sky_light(&self, x: i32, y: i32, z: i32, light: u8) -> Result<()> { ... }
    // #[napi]
    // pub fn set_biome_id(&self, x: i32, y: i32, z: i32, biome_id: u32) -> Result<()> { ... }

}

// Simple struct to return basic block info to JS
#[napi(object)]
pub struct BlockInfo {
    pub state_id: u32,
    pub light: u8,
    pub sky_light: u8,
    pub biome_id: u32,
}