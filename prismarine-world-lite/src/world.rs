// src/world.rs
use crate::chunk::ChunkColumn;
use crate::coords::{ChunkCoords, WorldCoords, MIN_SECTION_Y, SECTION_COUNT};
use crate::parsing::parse_chunk_section;
use crate::raycast::{intersect_aabb, RaycastIterator, RaycastResult, Vec3Arg, BlockFace};
use glam::DVec3;
// Import CollisionShapeIds from the correct module
use minecraft_data_rs::models::block_collision_shapes::CollisionShapeIds;
use minecraft_data_rs::api::{versions_by_minecraft_version, Api};
use napi::bindgen_prelude::*;
use napi_derive::napi;
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::{Arc, RwLock};
use dashmap::DashMap;

#[napi(js_name = "World")]
pub struct NapiWorld {
    columns: Arc<DashMap<ChunkCoords, Arc<RwLock<ChunkColumn>>>>,
    mc_data_api: Arc<Api>,
}

#[napi]
impl NapiWorld {
    #[napi(factory)]
    pub fn with_version(version_string: String) -> Result<Self> {
        let versions = versions_by_minecraft_version()
            .map_err(|e| napi::Error::new(
                napi::Status::GenericFailure,
                format!("Failed to get Minecraft versions: {}", e)
            ))?;

        let version = versions.get(&version_string)
            .ok_or_else(|| napi::Error::new(
                napi::Status::InvalidArg,
                format!("Unsupported Minecraft version: {}", version_string)
            ))?;

        let api = Api::new(version.clone());

        Ok(NapiWorld {
            columns: Arc::new(DashMap::new()),
            mc_data_api: Arc::new(api),
        })
    }

    /// Loads chunk column data from a network buffer (like `map_chunk` packet data).
    #[napi]
    pub fn load_column(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        data_buffer: Buffer,
    ) -> Result<()> {
        let coords = ChunkCoords { x: chunk_x, z: chunk_z };
        let mut cursor = Cursor::new(data_buffer.as_ref());
        let mut column = ChunkColumn::new();
        eprintln!("[load_column] Loading chunk ({}, {}), Buffer length: {}", chunk_x, chunk_z, data_buffer.len());

        for i in 0..SECTION_COUNT {
             let section_y = MIN_SECTION_Y + i as i32;
             let cursor_before = cursor.position();
             // eprintln!("[load_column] Attempting to parse section y={}, cursor at: {}", section_y, cursor_before); // Optional

             if cursor_before < cursor.get_ref().len() as u64 {
                match parse_chunk_section(&mut cursor, section_y) { // Pass section_y
                    Ok(section) => {
                        // eprintln!("[load_column] Parsed section y={}, bytes read: {}", section_y, cursor.position() - cursor_before); // Optional
                        column.insert_section(section_y, section);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                        eprintln!("[load_column] Reached EOF while parsing section y={}, stopping.", section_y);
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
                 eprintln!("[load_column] No more data in buffer for section y={}, stopping.", section_y);
                 break;
             }
        }
        eprintln!("[load_column] Finished parsing sections for ({}, {}), final cursor at: {}", chunk_x, chunk_z, cursor.position());

        self.columns.insert(coords, Arc::new(RwLock::new(column)));
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


    /// Performs a raycast from the origin in the given direction.
    ///
    /// Args:
    /// - `origin`: `{ x: number, y: number, z: number }` - The starting point of the ray.
    /// - `direction`: `{ x: number, y: number, z: number }` - The direction vector of the ray (should be normalized).
    /// - `max_distance`: `number` - The maximum distance the ray should travel.
    /// - `intersect_non_solid_block_names`: `string[] | null` - Optional array of block names (e.g., "water", "grass") to intersect even if they are not solid.
    ///
    /// Returns:
    /// - `object | null`: An object containing `position` (block coords), `face` (number), and `intersect_point` (exact coords), or `null` if no intersection is found within the distance.
    #[napi(ts_args_type = "origin: { x: number, y: number, z: number }, direction: { x: number, y: number, z: number }, max_distance: number, intersect_non_solid_block_names?: string[] | null")]
    pub fn raycast(
        &self,
        origin_arg: Vec3Arg,
        direction_arg: Vec3Arg,
        max_distance: f64,
        intersect_non_solid_block_names: Option<Vec<String>>,
    ) -> Option<RaycastResult> {
        let origin = DVec3::from(origin_arg);
        let direction = DVec3::from(direction_arg);

        let direction = direction.normalize_or_zero();
        if direction == DVec3::ZERO { return None; }

        let inv_dir = DVec3::new(1.0 / direction.x, 1.0 / direction.y, 1.0 / direction.z);

        let mut iterator = RaycastIterator::new(origin, direction, max_distance);
        let non_solid_exceptions: HashSet<String> = intersect_non_solid_block_names
            .unwrap_or_default()
            .into_iter()
            .collect();

        // Load collision shapes data once
        let collision_shapes_data = match self.mc_data_api.blocks.block_collision_shapes() {
            Ok(data) => data,
            Err(_) => return None, // Or handle error appropriately
        };

        let mut closest_hit: Option<(f64, BlockFace, WorldCoords)> = None;

        while let Some((block_pos, _entered_face)) = iterator.next() {
            let state_id = self.get_block_state_id(block_pos.x, block_pos.y, block_pos.z);
            if state_id == 0 { continue; } // Skip air

            // Use unwrap here assuming minecraft-data is consistent for the loaded version
            if let Some(block_data) = self.mc_data_api.blocks.blocks_by_state_id().unwrap().get(&(state_id as u32)) {
                // FIX 1: Use matches! for BoundingBox comparison
                let is_solid = matches!(block_data.bounding_box, minecraft_data_rs::models::block::BoundingBox::Block);
                let is_exception = non_solid_exceptions.contains(&block_data.name);

                if is_solid || is_exception {
                    let block_world_pos = DVec3::new(block_pos.x as f64, block_pos.y as f64, block_pos.z as f64);
                    let mut hit_in_this_block = false;

                    // FIX 2: Get shapes from collision_shapes_data
                    if let Some(shape_ids) = collision_shapes_data.blocks.get(&block_data.name) {
                        let ids_to_check = match shape_ids {
                            CollisionShapeIds::Value(id) => vec![*id],
                            CollisionShapeIds::Array(ids) => ids.clone(),
                        };

                        for shape_id in ids_to_check {
                            if let Some(shape_vec) = collision_shapes_data.shapes.get(&shape_id) {
                                for shape in shape_vec {
                                    // Shape coords are relative 0-1, convert to world AABB
                                    let aabb_min = block_world_pos + DVec3::new(shape[0] as f64, shape[1] as f64, shape[2] as f64);
                                    let aabb_max = block_world_pos + DVec3::new(shape[3] as f64, shape[4] as f64, shape[5] as f64);

                                    if let Some((t, face)) = intersect_aabb(aabb_min, aabb_max, origin, inv_dir) {
                                        if t >= 0.0 && t * t * direction.length_squared() <= iterator.max_distance_sq { // Check distance using t
                                            if closest_hit.is_none() || t < closest_hit.unwrap().0 {
                                                closest_hit = Some((t, face, block_pos));
                                                hit_in_this_block = true; // Mark that we found a hit within this block's shapes
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Fallback for solid blocks if no specific shapes were found or intersected
                    // Only do this if we didn't already find a hit within specific shapes for this block
                    if !hit_in_this_block && is_solid {
                         let aabb_min = block_world_pos;
                         let aabb_max = block_world_pos + DVec3::ONE;
                         if let Some((t, face)) = intersect_aabb(aabb_min, aabb_max, origin, inv_dir) {
                             if t >= 0.0 && t * t * direction.length_squared() <= iterator.max_distance_sq {
                                 if closest_hit.is_none() || t < closest_hit.unwrap().0 {
                                     closest_hit = Some((t, face, block_pos));
                                     hit_in_this_block = true;
                                 }
                             }
                         }
                    }

                    // Optimization: If the closest hit found is closer than the current ray position, stop.
                    if hit_in_this_block && closest_hit.unwrap().0 < iterator.current_t {
                        break; // Break outer loop
                    }
                }
            }
        }

        // Construct result from the closest hit found
        closest_hit.map(|(t, face, block_pos)| {
            RaycastResult {
                position: block_pos,
                face: face as u32,
                intersect_point: (origin + direction * t).into(),
            }
        })
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