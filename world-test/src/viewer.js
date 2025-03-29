// world-test/src/protocol.js
const mc = require('minecraft-protocol');
const { World } = require('../../prismarine-world-lite/prismarine-world-lite.node');
const { Vec3 } = require('vec3');
const PrismarineWorld = require('prismarine-world')('1.21.1'); // Standard prismarine-world
const PrismarineChunk = require('prismarine-chunk')('1.21.1'); // Standard prismarine-chunk
const { standalone } = require('prismarine-viewer'); // Import prismarine-viewer

const HOST = process.argv[2] || 'localhost';
const PORT = parseInt(process.argv[3] || '25565', 10);
const USERNAME = process.argv[4] || 'NapiBot';
const VERSION = '1.21.1'; // Match the version your Rust world supports
const ChatMessage = require('prismarine-chat')(VERSION);

console.log(`Connecting to ${HOST}:${PORT} as ${USERNAME} for MC ${VERSION}...`);

// 1. Create the Rust World instance
const world = new World();
console.log('Rust World instance created.');

// --- State Tracking ---
let botPosition = null; // Variable to store the bot's position
let viewerInstance = null; // To hold the viewer instance
// --------------------

// 2. Create the minecraft-protocol client
const client = mc.createClient({
  host: HOST,
  port: PORT,
  username: USERNAME,
  version: VERSION,
  auth: 'offline',
});

// --- Packet Listeners for World ---

client.on('map_chunk', (packet) => {
  try {
    // console.log(`Received chunk ${packet.x}, ${packet.z}`);
    world.loadColumn(packet.x, packet.z, packet.chunkData);
    // console.log(`Loaded chunk ${packet.x}, ${packet.z} into Rust world`);
  } catch (e) {
    console.error(`Error loading chunk ${packet.x}, ${packet.z}:`, e);
  }
});

client.on('unload_chunk', (packet) => {
  try {
    // console.log(`Unloading chunk ${packet.chunkX}, ${packet.chunkZ}`);
    world.unloadColumn(packet.chunkX, packet.chunkZ);
    // console.log(`Unloaded chunk ${packet.chunkX}, ${packet.chunkZ} from Rust world`);
  } catch (e) {
    console.error(`Error unloading chunk ${packet.chunkX}, ${packet.chunkZ}:`, e);
  }
});

client.on('block_change', (packet) => {
  try {
    const pos = packet.location;
    // console.log(`Single block change at ${pos.x}, ${pos.y}, ${pos.z} to ${packet.type}`);
    world.setBlockStateId(pos.x, pos.y, pos.z, packet.type);
    // console.log(`Updated single block in Rust world.`);
  } catch (e) {
    console.error(`Error setting block state ID for single block change:`, e);
  }
});

// Multi Block Update (1.16.2+)
client.on('multi_block_change', (packet) => {
  let chunkX, chunkZ, sectionY;
  if (packet.chunkCoordinates) { // 1.20.2+ structure
      chunkX = packet.chunkCoordinates.x;
      chunkZ = packet.chunkCoordinates.z;
      sectionY = packet.chunkCoordinates.y; // Section Y index
  } else { // Older structure
      chunkX = packet.chunkX;
      chunkZ = packet.chunkZ;
      sectionY = -1; // Needs careful handling if this path is taken for 1.21.1
      console.warn("Received older multi_block_change format, section Y calculation might be inaccurate for 1.21.1");
  }

  // console.log(`Multi block change in chunk ${chunkX}, ${chunkZ}, sectionY: ${sectionY}`);

  for (const record of packet.records) {
    try {
        const recordBigInt = BigInt(record);
        const blockIndex = recordBigInt >> 12n;
        const posData = recordBigInt & 0xFFFn;

        const relativeY = Number(posData & 0xFn);
        const z = Number((posData >> 4n) & 0xFn);
        const x = Number((posData >> 8n) & 0xFn);

        const worldX = chunkX * 16 + x;
        const MIN_SECTION_Y_JS = -4;
        const absoluteSectionY = MIN_SECTION_Y_JS + sectionY;
        const worldY = (absoluteSectionY * 16) + relativeY;
        const worldZ = chunkZ * 16 + z;
        const newStateId = Number(blockIndex);

        // console.log(`  Updating ${worldX}, ${worldY}, ${worldZ} to state ${newStateId}`);
        world.setBlockStateId(worldX, worldY, worldZ, newStateId);

    } catch (e) {
        console.error(`Error setting block state ID for multi block change record (${record}):`, e);
    }
  }
  // console.log(`Finished multi block change in Rust world.`);
});

// --- Position Tracking ---
client.on('position', (packet) => {
    botPosition = new Vec3(packet.x, packet.y, packet.z);
    // console.log(`Position updated by server: ${botPosition}`);
    if (packet.teleportId !== undefined) {
        client.write('teleport_confirm', {
            teleportId: packet.teleportId
        });
    }
     client.write('position', {
       x: packet.x,
       y: packet.y,
       z: packet.z,
       onGround: true
     });
     // Update viewer center if it exists
     if (viewerInstance) {
        viewerInstance.setFirstPersonCamera(botPosition.offset(0, 1.6, 0), client.yaw, client.pitch);
     }
});

// --- Bot Logic ---

client.on('login', () => {
  console.log(`Successfully logged in as ${client.username}`);
});

client.on('spawn', () => {
  console.log('Bot has spawned in the world.');
  client.chat('Hello world! I am using a Rust NAPI world.');

  // Periodically check the block the bot is standing on
  setInterval(() => {
    if (!botPosition) {
        // console.log("[Interval] Waiting for initial position...");
        return;
    }
    const feetPos = botPosition.floored().offset(0, -1, 0);
    try {
      const block = world.getBlock(feetPos.x, feetPos.y, feetPos.z);
      if (block) {
        // console.log(`[Rust World] Standing on block with state ID: ${block.stateId} (Biome ID: ${block.biomeId}, Light: ${block.light}, SkyLight: ${block.skyLight})`);
      } else {
        // console.log(`[Rust World] Standing on unloaded chunk at ${feetPos.x}, ${feetPos.y}, ${feetPos.z}`);
      }
    } catch (e) {
      console.error(`[Rust World] Error getting block at ${feetPos.x}, ${feetPos.y}, ${feetPos.z}:`, e);
    }
  }, 5000); // Check every 5 seconds
});

// --- Chat Handling ---

client.on('playerChat', (data) => {
    const sender = data.senderName ? ChatMessage.fromNotch(data.senderName).toString() : (data.sender?.toString() || 'Unknown');
    const message = data.plainMessage;
    console.log(`<${sender}> ${message}`);
    if (message.trim() === '!getblock') {
        handleGetBlockCommand();
    } else if (message.trim() === '!exportWorld') {
        handleExportWorldCommand();
    }
});

client.on('systemChat', (data) => {
    const message = ChatMessage.fromNotch(data.formattedMessage).toString();
    console.log(`[System] ${message}`);
});

function handleGetBlockCommand() {
    if (!botPosition) {
        client.chat("I don't know where I am yet!");
        return;
    }
    const feetPos = botPosition.floored().offset(0, -1, 0);
    try {
        const block = world.getBlock(feetPos.x, feetPos.y, feetPos.z);
        if (block) {
            client.chat(`[Rust World] Block below me: StateID=${block.stateId}, BiomeID=${block.biomeId}, Light=${block.light}, SkyLight=${block.skyLight}`);
        } else {
            client.chat(`[Rust World] Chunk below me is not loaded.`);
        }
    } catch (e) {
        client.chat(`[Rust World] Error getting block: ${e.message}`);
        console.error(e);
    }
}

// --- Export World Command Handler ---
async function handleExportWorldCommand() {
    if (!botPosition) {
        client.chat("I don't know where I am yet!");
        return;
    }

    client.chat(`Exporting loaded chunks from Rust world...`);
    console.log(`[Export] Starting export...`);

    // Create standard prismarine instances
    const viewerWorld = new PrismarineWorld(null); // No generator needed, we load data

    const MIN_SECTION_Y_JS = -4;
    const MAX_SECTION_Y_JS = 19;
    const SECTION_VOLUME = 16 * 16 * 16;

    let totalChunksExported = 0;
    let totalSectionsLoaded = 0;

    try {
        const loadedChunksCoords = world.getLoadedChunks(); // Get coords from Rust
        console.log(`[Export] Found ${loadedChunksCoords.length} loaded chunks in Rust world.`);

        if (loadedChunksCoords.length === 0) {
            client.chat("No chunks loaded in Rust world to export.");
            return;
        }

        for (const { x: chunkX, z: chunkZ } of loadedChunksCoords) {
            // console.log(`[Export] Processing chunk ${chunkX}, ${chunkZ}`);
            const viewerChunk = new PrismarineChunk({ minY: -64, worldHeight: 384 }); // Match 1.18+ dimensions
            let sectionsLoadedInChunk = 0;

            for (let sectionY = MIN_SECTION_Y_JS; sectionY <= MAX_SECTION_Y_JS; sectionY++) {
                const stateIdBuffer = world.exportSectionStates(chunkX, chunkZ, sectionY);

                if (stateIdBuffer) {
                    if (stateIdBuffer.length !== SECTION_VOLUME * 4) {
                        console.warn(`[Export] Unexpected buffer length for section Y=${sectionY} in chunk ${chunkX},${chunkZ}. Expected ${SECTION_VOLUME * 4}, got ${stateIdBuffer.length}. Skipping.`);
                        continue;
                    }

                    const stateIds = new Uint32Array(SECTION_VOLUME);
                    for (let i = 0; i < SECTION_VOLUME; i++) {
                        stateIds[i] = stateIdBuffer.readUInt32LE(i * 4);
                    }

                    for (let yRel = 0; yRel < 16; yRel++) {
                        for (let zRel = 0; zRel < 16; zRel++) {
                            for (let xRel = 0; xRel < 16; xRel++) {
                                const index = (yRel * 16 + zRel) * 16 + xRel;
                                const stateId = stateIds[index];
                                const absoluteY = (sectionY * 16) + yRel;
                                viewerChunk.setBlockStateId(new Vec3(xRel, absoluteY, zRel), stateId);
                            }
                        }
                    }
                    sectionsLoadedInChunk++;
                }
            }

            // Set the populated chunk in the viewer world
            viewerWorld.setColumn(chunkX, chunkZ, viewerChunk);
            // console.log(`[Export] Finished exporting chunk ${chunkX}, ${chunkZ}. ${sectionsLoadedInChunk} sections loaded.`);
            totalChunksExported++;
            totalSectionsLoaded += sectionsLoadedInChunk;
        }

        console.log(`[Export] Finished exporting. Total chunks: ${totalChunksExported}, Total sections: ${totalSectionsLoaded}.`);
        client.chat(`Exported ${totalChunksExported} chunks (${totalSectionsLoaded} sections) successfully!`);

        // --- Viewer Integration ---
        if (viewerInstance) {
            console.log('[Viewer] Closing existing viewer instance.');
            // viewerInstance.close(); // Assuming viewer has a close method
            viewerInstance = null;
        }

        console.log('[Viewer] Starting prismarine-viewer...');
        const viewerPort = 3000; // Or choose another port
        viewerInstance = standalone({
            version: VERSION,
            world: viewerWorld,
            center: botPosition || new Vec3(0, 80, 0), // Center on bot or default
            port: viewerPort
        });
        console.log(`[Viewer] Viewer started on http://localhost:${viewerPort}`);
        client.chat(`Viewer started on http://localhost:${viewerPort}`);
        // You might want to update the viewer center periodically if the bot moves
        // viewerInstance.setFirstPersonCamera(botPosition.offset(0, 1.6, 0), client.yaw, client.pitch);

    } catch (e) {
        console.error(`[Export] Error during export:`, e);
        client.chat(`Error during export: ${e.message}`);
    }
}
// --- End Export World Command Handler ---


// --- End Chat Handling ---

// Handle disconnection and errors
client.on('end', (reason) => {
  console.log(`Disconnected: ${reason}`);
  if (viewerInstance) {
    console.log('[Viewer] Closing viewer due to disconnect.');
    // viewerInstance.close(); // Assuming viewer has a close method
    viewerInstance = null;
  }
});

client.on('error', (err) => {
  console.error('Connection Error:', err);
   if (viewerInstance) {
    console.log('[Viewer] Closing viewer due to error.');
    // viewerInstance.close(); // Assuming viewer has a close method
    viewerInstance = null;
  }
});