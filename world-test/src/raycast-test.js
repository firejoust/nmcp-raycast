// world-test/src/protocol-raycast.js
const mc = require('minecraft-protocol');
const { World } = require('../../prismarine-world-lite/prismarine-world-lite.node');
const { Vec3 } = require('vec3');

const HOST = process.argv[2] || 'localhost';
const PORT = parseInt(process.argv[3] || '25565', 10);
const USERNAME = process.argv[4] || 'NapiRayBot';
const VERSION = '1.21.1';
const ChatMessage = require('prismarine-chat')(VERSION);

console.log(`Connecting to ${HOST}:${PORT} as ${USERNAME} for MC ${VERSION}...`);

let world;
try {
    world = World.withVersion(VERSION);
    console.log('Rust World instance created for version', VERSION);
} catch (e) {
    console.error("Failed to initialize Rust World:", e);
    process.exit(1);
}

// --- State Tracking ---
let botPosition = null;
let botYaw = 0;
let botPitch = 0;
const DEFAULT_EYE_HEIGHT = 1.62;
// --------------------

const client = mc.createClient({
  host: HOST,
  port: PORT,
  username: USERNAME,
  version: VERSION,
  auth: 'offline',
});

// --- Packet Listeners for World (Keep existing listeners) ---
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

client.on('multi_block_change', (packet) => {
  let chunkX, chunkZ, sectionY;
  if (packet.chunkCoordinates) {
      chunkX = packet.chunkCoordinates.x;
      chunkZ = packet.chunkCoordinates.z;
      sectionY = packet.chunkCoordinates.y;
  } else {
      chunkX = packet.chunkX;
      chunkZ = packet.chunkZ;
      sectionY = -1;
      console.warn("Received older multi_block_change format, Y coordinate might be incorrect if sectionY is needed.");
  }

  for (const record of packet.records) {
    try {
        const recordBigInt = BigInt(record);
        const blockStateId = Number(recordBigInt >> 12n);
        const posData = recordBigInt & 0xFFFn;

        const relativeY = Number(posData & 0xFn);
        const z = Number((posData >> 4n) & 0xFn);
        const x = Number((posData >> 8n) & 0xFn);

        const worldX = chunkX * 16 + x;
        const MIN_CHUNK_Y_RUST = -64;
        const worldY = (sectionY * 16) + MIN_CHUNK_Y_RUST + relativeY;
        const worldZ = chunkZ * 16 + z;

        world.setBlockStateId(worldX, worldY, worldZ, blockStateId);

    } catch (e) {
        console.error(`Error setting block state ID for multi block change record (${record}):`, e);
    }
  }
});


// --- Position and Look Tracking ---
client.on('position', (packet) => {
    botPosition = new Vec3(packet.x, packet.y, packet.z);
    botYaw = packet.yaw;
    botPitch = packet.pitch;

    if (packet.teleportId !== undefined) {
        client.write('teleport_confirm', { teleportId: packet.teleportId });
    }
    client.write('position_look', {
       x: packet.x,
       y: packet.y,
       z: packet.z,
       yaw: packet.yaw,
       pitch: packet.pitch,
       onGround: true
     });
});

// --- Bot Logic ---
client.on('login', () => {
  console.log(`Successfully logged in as ${client.username}`);
});

client.on('spawn', () => {
  console.log('Bot has spawned in the world.');
  client.chat('Hello world! I am using a Rust NAPI world with raycasting. Type !getblock or !raycast');
});

// --- Chat Handling ---
client.on('playerChat', (data) => {
    const sender = data.senderName ? ChatMessage.fromNotch(data.senderName).toString() : (data.sender?.toString() || 'Unknown');
    const message = data.plainMessage;
    console.log(`<${sender}> ${message}`);

    const command = message.trim().toLowerCase();

    if (command === '!getblock') {
        handleGetBlockCommand(); // Call the updated handler
    } else if (command === '!raycast') {
        handleRaycastCommand();
    }
});

client.on('systemChat', (data) => {
    const message = ChatMessage.fromNotch(data.formattedMessage).toString();
    console.log(`[System] ${message}`);
});

// --- Updated handleGetBlockCommand ---
function handleGetBlockCommand() {
    if (!botPosition) {
        client.chat("I don't know where I am yet!");
        return;
    }
    const feetPos = botPosition.floored().offset(0, -1, 0);
    // Calculate chunk coordinates from feetPos
    const chunkX = Math.floor(feetPos.x / 16);
    const chunkZ = Math.floor(feetPos.z / 16);

    try {
        const block = world.getBlock(feetPos.x, feetPos.y, feetPos.z);
        if (block) {
            // Include chunk coordinates in the message
            client.chat(`[Rust World] Block below me at (${feetPos.x}, ${feetPos.y}, ${feetPos.z}) in chunk (${chunkX}, ${chunkZ}): StateID=${block.stateId}, BiomeID=${block.biomeId}, Light=${block.light}, SkyLight=${block.skyLight}`);
        } else {
            // Include chunk coordinates in the message
            client.chat(`[Rust World] Chunk below me (${chunkX}, ${chunkZ}) is not loaded.`);
        }
    } catch (e) {
        client.chat(`[Rust World] Error getting block: ${e.message}`);
        console.error(e);
    }
}
// --- End Updated handleGetBlockCommand ---

// --- Raycast Command Handler (Keep existing) ---
const blockFaceNames = {
    0: 'bottom (-Y)', 1: 'top (+Y)', 2: 'north (-Z)',
    3: 'south (+Z)', 4: 'west (-X)', 5: 'east (+X)',
};

function handleRaycastCommand() {
    if (!botPosition) {
        client.chat("I'm not ready yet (missing position).");
        return;
    }
    const eyeHeight = DEFAULT_EYE_HEIGHT;
    const originVec = botPosition.offset(0, eyeHeight, 0);
    const currentYaw = botYaw;
    const currentPitch = botPitch;

    const directionVec = new Vec3(
        -Math.sin(currentYaw) * Math.cos(currentPitch),
        Math.sin(currentPitch),
        -Math.cos(currentYaw) * Math.cos(currentPitch)
    );
    const maxDistance = 100.0;

    console.log(`Raycasting from ${originVec} in direction ${directionVec.normalize()} for ${maxDistance} blocks...`);

    try {
        const hit = world.raycast(
            { x: originVec.x, y: originVec.y, z: originVec.z },
            { x: directionVec.x, y: directionVec.y, z: directionVec.z },
            maxDistance,
            null
        );

        if (hit) {
            const faceName = blockFaceNames[hit.face] || `Unknown (${hit.face})`;
            const blockPos = hit.position;
            const intersectPos = hit.intersectPoint;
            const blockInfo = world.getBlock(blockPos.x, blockPos.y, blockPos.z);
            const blockName = blockInfo ? `StateID=${blockInfo.stateId}` : 'Unknown';

            client.chat(`[Raycast Hit] Block: ${blockName} at (${blockPos.x}, ${blockPos.y}, ${blockPos.z}). Face: ${faceName}. Intersect: (${intersectPos.x.toFixed(2)}, ${intersectPos.y.toFixed(2)}, ${intersectPos.z.toFixed(2)})`);
        } else {
            client.chat('[Raycast Miss] No block found within range.');
        }
    } catch (e) {
        client.chat(`[Raycast Error] ${e.message}`);
        console.error("Raycast Error:", e);
    }
}
// --- End Raycast Command Handler ---

// --- End Chat Handling ---

// Handle disconnection and errors
client.on('end', (reason) => {
  console.log(`Disconnected: ${reason}`);
});

client.on('error', (err) => {
  console.error('Connection Error:', err);
});