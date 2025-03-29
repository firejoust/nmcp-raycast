/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

/** N-API accessible struct representing chunk coordinates (X, Z). */
export interface JsChunkCoords {
  x: number
  z: number
}
/** Simple struct to return basic block information to Node.js. */
export interface BlockInfo {
  /** The global block state ID. */
  stateId: number
  /** The block light level (0-15). Placeholder, currently always 15. */
  light: number
  /** The sky light level (0-15). Placeholder, currently always 15. */
  skyLight: number
  /** The global biome ID. */
  biomeId: number
}
export type NapiWorld = World
/**
 * The main world class exposed to Node.js via N-API.
 * Manages chunk columns and provides methods for block/biome access.
 */
export declare class World {
  /** Creates a new, empty world instance. */
  constructor()
  /**
   * Loads chunk column data from a network buffer (like the `map_chunk` packet data).
   * Parses block states and biomes for all sections present in the buffer.
   *
   * # Arguments
   * * `chunk_x`, `chunk_z`: The coordinates of the chunk column to load.
   * * `data_buffer`: A Node.js `Buffer` containing the serialized chunk data.
   */
  loadColumn(chunkX: number, chunkZ: number, dataBuffer: Buffer): void
  /** Unloads a chunk column from memory. */
  unloadColumn(chunkX: number, chunkZ: number): void
  /**
   * Gets the global state ID of the block at the given world coordinates.
   * Returns 0 (air) if the chunk or section is not loaded or if a lock fails.
   */
  getBlockStateId(x: number, y: number, z: number): number
  /**
   * Sets the global state ID of the block at the given world coordinates.
   * Creates the chunk section if it doesn't exist and `state_id` is not 0.
   * Returns an error if the chunk is not loaded or the write lock cannot be acquired.
   */
  setBlockStateId(x: number, y: number, z: number, stateId: number): void
  /**
   * Gets a simplified block object containing state ID, light levels, and biome ID.
   * Returns `None` (-> `null` in JS) if the chunk is not loaded.
   * Returns default values (air, max light) if the lock fails.
   */
  getBlock(x: number, y: number, z: number): BlockInfo | null
  /**
   * Gets the block light level at the given world coordinates.
   * Returns 0 if the chunk is not loaded or if a lock fails.
   */
  getBlockLight(x: number, y: number, z: number): number
  /**
   * Gets the sky light level at the given world coordinates.
   * Returns 15 (max) if the chunk is not loaded or if a lock fails.
   */
  getSkyLight(x: number, y: number, z: number): number
  /**
   * Gets the global biome ID at the given world coordinates.
   * Returns 0 if the chunk is not loaded or if a lock fails.
   */
  getBiomeId(x: number, y: number, z: number): number
  /**
   * Exports the block state IDs for a single chunk section as a Node.js `Buffer`.
   * Returns `null` if the chunk or section is not loaded, or if the read lock fails.
   */
  exportSectionStates(chunkX: number, chunkZ: number, sectionY: number): Buffer | null
  /** Returns a list of coordinates for all currently loaded chunks. */
  getLoadedChunks(): { x: number; z: number; }[]
}
