const mc = require('minecraft-protocol');
const { World } = require('../prismarine-world-lite/prismarine-world-lite.node'); // Assuming your built addon is index.js
const { Vec3 } = require('vec3'); // Import Vec3

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
    world.loadColumn(packet.x, packet.z, packet.chunkData);
  } catch (e) {
    console.error(`Error loading chunk ${packet.x}, ${packet.z}:`, e);
  }
});

client.on('unload_chunk', (packet) => {
  try {
    world.unloadColumn(packet.chunkX, packet.chunkZ);
  } catch (e) {
    console.error(`Error unloading chunk ${packet.chunkX}, ${packet.chunkZ}:`, e);
  }
});

client.on('block_change', (packet) => {
  try {
    const pos = packet.location;
    world.setBlockStateId(pos.x, pos.y, pos.z, packet.type);
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
  }

  // console.log(`Multi block change in chunk ${chunkX}, ${chunkZ}, sectionY: ${sectionY}`);

  for (const record of packet.records) {
    try {
        // --- FIX: Ensure 'record' is treated as BigInt before operations ---
        const recordBigInt = BigInt(record); // Explicitly cast to BigInt
        const blockIndex = recordBigInt >> 12n;
        const posData = recordBigInt & 0xFFFn;
        // --- End FIX ---

        // Now use posData (which is definitely a BigInt) for further BigInt operations
        const relativeY = Number(posData & 0xFn);
        const z = Number((posData >> 4n) & 0xFn);
        const x = Number((posData >> 8n) & 0xFn);

        const worldX = chunkX * 16 + x;
        // Calculate absolute Y based on section Y
        const worldY = (sectionY * 16) + relativeY;
        const worldZ = chunkZ * 16 + z;
        const newStateId = Number(blockIndex); // Convert final result back to Number

        // console.log(`  Updating ${worldX}, ${worldY}, ${worldZ} to state ${newStateId}`);
        world.setBlockStateId(worldX, worldY, worldZ, newStateId);

    } catch (e) {
        // Add the record value to the log for better debugging
        console.error(`Error setting block state ID for multi block change record (${record}):`, e);
    }
  }
  // console.log(`Finished multi block change in Rust world.`);
});

// --- Position Tracking ---
client.on('position', (packet) => {
    // This packet sets the absolute position
    botPosition = new Vec3(packet.x, packet.y, packet.z);
    // console.log(`Position updated by server: ${botPosition}`);

    // Acknowledge the position update to the server if required by protocol version
    // For 1.12+, we need to send teleport_confirm
    if (packet.teleportId !== undefined) {
        client.write('teleport_confirm', {
            teleportId: packet.teleportId
        });
    }
    // Also send our own position packet back to confirm, helps prevent rubber-banding
     client.write('position', { // or position_look if you track yaw/pitch
       x: packet.x,
       y: packet.y,
       z: packet.z,
       onGround: true // Assume onGround after server sets position, adjust if needed
     });
});

// For 1.21.3+, listen for sync_entity_position instead of 'position'
// client.on('sync_entity_position', (packet) => {
//     if (packet.entityId === client.entityId) { // Ensure it's our own entity
//         botPosition = new Vec3(packet.x, packet.y, packet.z);
//         console.log(`Position updated by server (sync): ${botPosition}`);
//         // Acknowledge if needed (check protocol specifics, might not be needed for sync)
//         // Send our own position packet back
//         client.write('sync_player_position', {
//             x: packet.x,
//             y: packet.y,
//             z: packet.z,
//             yaw: packet.yaw, // Use server-provided yaw/pitch
//             pitch: packet.pitch,
//             onGround: true // Assume onGround, adjust if needed
//         });
//     }
// });

// --- Bot Logic ---

client.on('login', () => {
  console.log(`Successfully logged in as ${client.username}`);
});

client.on('spawn', () => {
  console.log('Bot has spawned in the world.');
  client.chat('Hello world! I am using a Rust NAPI world.');

  // Periodically check the block the bot is standing on
  setInterval(() => {
    // Use the tracked botPosition
    if (!botPosition) {
        console.log("[Interval] Waiting for initial position...");
        return;
    }

    const feetPos = botPosition.floored().offset(0, -1, 0); // Block below feet

    try {
      const block = world.getBlock(feetPos.x, feetPos.y, feetPos.z);
      if (block) {
        console.log(`[Rust World] Standing on block with state ID: ${block.stateId} (Biome ID: ${block.biomeId}, Light: ${block.light}, SkyLight: ${block.skyLight})`);
      } else {
        console.log(`[Rust World] Standing on unloaded chunk at ${feetPos.x}, ${feetPos.y}, ${feetPos.z}`);
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
    }
});

client.on('systemChat', (data) => {
    const message = ChatMessage.fromNotch(data.formattedMessage).toString();
    console.log(`[System] ${message}`);
});

function handleGetBlockCommand() {
    // Use the tracked botPosition
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

// --- End Chat Handling ---

// Handle disconnection and errors
client.on('end', (reason) => {
  console.log(`Disconnected: ${reason}`);
});

client.on('error', (err) => {
  console.error('Connection Error:', err);
});