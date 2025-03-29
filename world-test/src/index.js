const { World } = require('../../prismarine-world-lite/prismarine-world-lite.node'); // Assuming your built addon is index.js
const assert = require('assert');

// Example usage simulating minecraft-protocol events

// 1. Create a world instance
const world = new World();
console.log('Created World instance');

// 2. Simulate receiving chunk data (replace with actual buffer from minecraft-protocol)
//    This is a placeholder buffer - you need real data from the game.
const chunkX = 0;
const chunkZ = 0;
// Example: A very simple section (mostly air, single value palette for blocks and biomes)
const simpleSectionBuffer = Buffer.from([
    0x00, 0x01, // solid block count = 1
    0x00,       // block bits per entry = 0 (single value palette)
    0x01,       // block palette value = 1 (e.g., stone state ID)
    0x00,       // block data length = 0
    0x00,       // biome bits per entry = 0 (single value palette)
    0x01,       // biome palette value = 1 (e.g., plains biome ID)
    0x00        // biome data length = 0
]);

const numSections = 24; // For y = -64 to y = 319
const chunkDataBuffers = Array(numSections).fill(simpleSectionBuffer);
const fullChunkDataBuffer = Buffer.concat(chunkDataBuffers);

try {
    // Use camelCase: loadColumn instead of load_column
    world.loadColumn(chunkX, chunkZ, fullChunkDataBuffer);
    console.log(`Loaded chunk column at ${chunkX}, ${chunkZ}`);

    // 3. Get a block
    // Use camelCase: getBlock instead of get_block
    const blockAir = world.getBlock(5, 65, 5); // Coords within the loaded chunk
    console.log('Block at 5, 65, 5:', blockAir);
    // state_id becomes stateId in JS object
    assert(blockAir.stateId === 0 || blockAir.stateId === 1, 'Expected air or stone from simple buffer');

    // 4. Set a block
    const newStateId = 1; // Example: Stone state ID
    // Use camelCase: setBlockStateId instead of set_block_state_id
    world.setBlockStateId(5, 65, 5, newStateId);
    console.log(`Set block at 5, 65, 5 to state ID ${newStateId}`);

    // 5. Get the block again to verify
    // Use camelCase: getBlock instead of get_block
    const blockStone = world.getBlock(5, 65, 5);
    console.log('Block at 5, 65, 5 after set:', blockStone);
    // state_id becomes stateId
    assert.strictEqual(blockStone?.stateId, newStateId, 'Block state ID should have been updated');

    // 6. Get biome
    // Use camelCase: getBiomeId instead of get_biome_id
    const biomeId = world.getBiomeId(5, 65, 5);
    console.log(`Biome ID at 5, 65, 5: ${biomeId}`);
    // biome_id becomes biomeId
    assert.strictEqual(biomeId, 1, 'Expected biome ID 1 from simple buffer');

    // 7. Unload the chunk
    // Use camelCase: unloadColumn instead of unload_column
    world.unloadColumn(chunkX, chunkZ);
    console.log(`Unloaded chunk column at ${chunkX}, ${chunkZ}`);

    // 8. Verify unload
    // Use camelCase: getBlock instead of get_block
    const blockAfterUnload = world.getBlock(5, 65, 5);
    console.log('Block at 5, 65, 5 after unload:', blockAfterUnload);
    assert.strictEqual(blockAfterUnload, null, 'Chunk should be unloaded');

    console.log('All tests passed!');

} catch (e) {
    console.error("Error during world operations:", e);
}