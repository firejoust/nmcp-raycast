const mc = require("minecraft-protocol");
const { World } = require("../../prismarine-world-lite/prismarine-world-lite.node"); // Adjust require path if needed
const { Vec3 } = require("vec3");

const states = mc.states;

// --- Configuration ---
const TARGET_SERVER_HOST = process.argv[2] || "localhost";
const TARGET_SERVER_PORT = parseInt(process.argv[3] || "25565", 10);
const PROXY_PORT = parseInt(process.argv[4] || "25566", 10);
const MC_VERSION = process.argv[5] || "1.21.1"; // Important: Use a version supported by prismarine-world-lite's parsing

if (!TARGET_SERVER_HOST) {
  console.error("Usage: node proxy.js <target_host> [target_port] [proxy_port] [mc_version]");
  process.exit(1);
}

console.log(`Proxy listening on port ${PROXY_PORT}, connecting to ${TARGET_SERVER_HOST}:${TARGET_SERVER_PORT} (${MC_VERSION})`);

// --- Proxy Server Setup ---
const proxyServer = mc.createServer({
  "online-mode": false, // Offline mode for simplicity
  port: PROXY_PORT,
  version: MC_VERSION,
  keepAlive: false, // Let the target server handle keep-alives initially
});

proxyServer.on("login", (serverClient) => {
  console.log(`[ProxyServer] Client ${serverClient.username} (${serverClient.socket.remoteAddress}) connected to proxy.`);

  // --- World Instance ---
  const world = new World();
  let playerPosition = new Vec3(0, 0, 0);

  // --- Target Client Setup ---
  const targetClient = mc.createClient({
    host: TARGET_SERVER_HOST,
    port: TARGET_SERVER_PORT,
    username: serverClient.username,
    version: serverClient.version,
    keepAlive: false,
    auth: "offline",
  });

  console.log(`[Proxy] Attempting connection to target server ${TARGET_SERVER_HOST}:${TARGET_SERVER_PORT} for ${serverClient.username}...`);

  // --- Packet Forwarding & Handling ---

  // Client (Player) -> Proxy -> Target Server
  serverClient.on("packet", (data, meta) => {
    if (targetClient.state === states.PLAY && meta.state === states.PLAY) {
      targetClient.write(meta.name, data);
    }
  });

  // Target Server -> Proxy -> Client (Player)
  targetClient.on("packet", (data, meta) => {
    if (serverClient.state === states.PLAY && meta.state === states.PLAY) {
      try {
        handleWorldPacket(world, meta.name, data);
        handlePlayerPositionPacket(meta.name, data);
      } catch (error) {
        console.error(`[World Update Error] Failed to handle packet ${meta.name}:`, error);
        console.error(error.stack); // Log stack trace
        // try { console.error('Packet Data:', JSON.stringify(data)); } catch { console.error('Packet Data: (circular or too large to stringify)'); }
      }

      serverClient.write(meta.name, data);

      if (meta.name === "keep_alive") {
        targetClient.write("keep_alive", { keepAliveId: data.keepAliveId });
      }
    }
  });

  // --- World State Handling ---
  function handleWorldPacket(worldInstance, packetName, packetData) {
    switch (packetName) {
      case "map_chunk":
      case "level_chunk_with_light": {
        const dataBuffer = Buffer.isBuffer(packetData.chunkData) ? packetData.chunkData : Buffer.from(packetData.chunkData);
        worldInstance.loadColumn(packetData.x, packetData.z, dataBuffer);
        break;
      }

      case "unload_chunk": {
        worldInstance.unloadColumn(packetData.chunkX, packetData.chunkZ);
        break;
      }

      case "block_update": {
        const pos = packetData.location;
        const stateId = packetData.blockId;
        worldInstance.setBlockStateId(pos.x, pos.y, pos.z, stateId);
        break;
      }

      case "multi_block_change": {
        let chunkX, chunkZ, sectionY;
        if (packetData.chunkCoordinates) {
          // 1.20.2+ structure
          chunkX = packetData.chunkCoordinates.x;
          chunkZ = packetData.chunkCoordinates.z;
          sectionY = packetData.chunkCoordinates.y; // Section Y index
        } else {
          // Older structure
          chunkX = packetData.chunkX;
          chunkZ = packetData.chunkZ;
          sectionY = -1; // Needs careful handling if this path is taken for 1.21.1
        }

        // console.log(`Multi block change in chunk ${chunkX}, ${chunkZ}, sectionY: ${sectionY}`);

        for (const record of packetData.records) {
          try {
            // --- FIX: Ensure 'record' is treated as BigInt before operations ---
            const recordBigInt = BigInt(record); // Explicitly cast to BigInt
            const blockIndex = recordBigInt >> 12n;
            const posData = recordBigInt & 0xfffn;
            // --- End FIX ---

            // Now use posData (which is definitely a BigInt) for further BigInt operations
            const relativeY = Number(posData & 0xfn);
            const z = Number((posData >> 4n) & 0xfn);
            const x = Number((posData >> 8n) & 0xfn);

            const worldX = chunkX * 16 + x;
            // Calculate absolute Y based on section Y
            const worldY = sectionY * 16 + relativeY;
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
      }
    }
  }

  // --- Player Position Handling ---
  function handlePlayerPositionPacket(packetName, packetData) {
    if (packetName === "position" || packetName === "synchronize_player_position") {
      playerPosition.set(packetData.x, packetData.y, packetData.z);
    }
  }

  // --- Connection End/Error Handling ---
  let ended = false;
  function endConnection(side, reason) {
    if (ended) return;
    ended = true;
    console.log(`[Proxy] Connection ended by ${side}. Reason: ${reason}`);
    serverClient.end(reason || "Proxy connection ended.");
    targetClient.end(reason || "Proxy connection ended.");
  }

  serverClient.on("end", (reason) => endConnection("Client", reason));
  targetClient.on("end", (reason) => endConnection("Target Server", reason));

  serverClient.on("error", (err) => {
    console.error("[ProxyServer Error]", err);
    endConnection("Client Error", err.message);
  });
  targetClient.on("error", (err) => {
    console.error("[TargetClient Error]", err);
    endConnection("Target Server Error", err.message);
  });

  // --- Initial Login Forwarding ---
  targetClient.on("login", (packet) => {
    console.log("[Proxy] Logged into target server.");
    handlePlayerPositionPacket("position", packet);
  });

  targetClient.on("connect", () => {
    console.log("[Proxy] Successfully connected to target server.");
  });
});

proxyServer.on("error", (err) => {
  console.error("[ProxyServer Listen Error]", err);
});

proxyServer.on("listening", () => {
  console.log(`[ProxyServer] Listening on 127.0.0.1:${PROXY_PORT}`);
});
