// src/world.rs
use crate::chunk::ChunkColumn;
use crate::coords::{ChunkCoords, WorldCoords, section_y_to_section_idx, MIN_SECTION_Y, SECTION_COUNT, SECTION_HEIGHT, SECTION_WIDTH, SectionRelCoords};
use crate::parsing::parse_chunk_section;
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use dashmap::DashMap;
use byteorder::{LittleEndian, WriteBytesExt};

#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct JsChunkCoords {
    pub x: i32,
    pub z: i32,
}

#[napi(js_name = "World")]
pub struct NapiWorld {
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
             if cursor.position() < cursor.get_ref().len() as u64 {
                match parse_chunk_section(&mut cursor) {
                    Ok(section) => {
                        column.insert_section(section_y, section);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error parsing section y={} for chunk ({}, {}): {}", section_y, chunk_x, chunk_z, e);
                         return Err(napi::Error::new(
                             napi::Status::GenericFailure,
                             format!("Failed to parse chunk section at y={}: {}", section_y, e),
                         ));
                    }
                }
             } else {
                 break;
             }
        }

        self.columns.insert(coords, Arc::new(RwLock::new(column)));
        Ok(())
    }

    /// Unloads a chunk column.
    #[napi]
    pub fn unload_column(&self, chunk_x: i32, chunk_z: i32) {
        let coords = ChunkCoords { x: chunk_x, z: chunk_z };
        self.columns.remove(&coords);
    }

    /// Gets the state ID of the block at the given world coordinates.
    #[napi]
    pub fn get_block_state_id(&self, x: i32, y: i32, z: i32) -> u32 {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();
        self.columns
            .get(&chunk_coords)
            .map(|entry| {
                match entry.value().try_read() {
                    Ok(guard) => guard.get_block_state_id(coords),
                    Err(_) => 0
                }
            })
            .unwrap_or(0)
    }

    /// Sets the state ID of the block at the given world coordinates.
    #[napi]
    pub fn set_block_state_id(&self, x: i32, y: i32, z: i32, state_id: u32) -> Result<()> {
        let coords = WorldCoords { x, y, z };
        let chunk_coords = coords.to_chunk_coords();
        match self.columns.get_mut(&chunk_coords) {
            Some(mut entry) => {
                match entry.value_mut().try_write() {
                    Ok(mut guard) => {
                        guard.set_block_state_id(coords, state_id);
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

    /// Gets a simplified block object (stateId, light, skyLight, biomeId).
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
                 Err(_) => BlockInfo {
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
            .unwrap_or(15)
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
            .unwrap_or(0)
    }

    /// Exports the block state IDs for a single chunk section as a Buffer.
    /// Returns null if the chunk or section is not loaded.
    #[napi(ts_return_type = "Buffer | null")]
    pub fn export_section_states(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        section_y: i32, // Section Y index (e.g., -4 to 19 for 1.18+)
    ) -> Option<Buffer> {
        let chunk_coords = ChunkCoords { x: chunk_x, z: chunk_z };

        self.columns.get(&chunk_coords).and_then(|entry| {
            match entry.value().try_read() {
                Ok(column) => {
                    section_y_to_section_idx(section_y).and_then(|vec_idx| {
                        column.sections.get(vec_idx).and_then(|opt_section| {
                            opt_section.as_ref().map(|section| {
                                const SECTION_VOLUME: usize = SECTION_WIDTH as usize * SECTION_HEIGHT as usize * SECTION_WIDTH as usize;
                                // Removed unused state_ids Vec
                                let mut buffer_data = Vec::with_capacity(SECTION_VOLUME * 4);

                                for y_rel in 0..SECTION_HEIGHT {
                                    for z_rel in 0..SECTION_WIDTH {
                                        for x_rel in 0..SECTION_WIDTH {
                                            let coords = SectionRelCoords { x: x_rel, y: y_rel, z: z_rel };
                                            let state_id = section.get_block_state_id(coords);
                                            // state_ids.push(state_id); // Removed
                                            buffer_data.write_u32::<LittleEndian>(state_id).unwrap();
                                        }
                                    }
                                }
                                buffer_data.into()
                            })
                        })
                    })
                }
                Err(_) => None,
            }
        })
    }

    /// Returns a list of coordinates for all currently loaded chunks.
    #[napi(ts_return_type = "{ x: number; z: number; }[]")]
    pub fn get_loaded_chunks(&self) -> Vec<JsChunkCoords> {
        self.columns
            .iter()
            .map(|entry| JsChunkCoords {
                x: entry.key().x,
                z: entry.key().z,
            })
            .collect()
    }
}

// Simple struct to return basic block info to JS
#[napi(object)]
pub struct BlockInfo {
    pub state_id: u32,
    pub light: u8,
    pub sky_light: u8,
    pub biome_id: u32,
}