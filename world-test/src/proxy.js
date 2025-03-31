const mc = require('minecraft-protocol')
const { World } = require('../../prismarine-world-lite/prismarine-world-lite.node') // Adjust require path
const { Vec3 } = require('vec3')
const EventEmitter = require('events')

const states = mc.states

class Proxy extends EventEmitter {
  // Creates a new minecraft proxy instance
  constructor (options) {
    super()
    this.targetHost = options.targetHost
    this.targetPort = options.targetPort
    this.proxyPort = options.proxyPort
    this.version = options.version
    this.onlineMode = options.onlineMode ?? false
    // Explicitly disable internal keep-alive handling for proxy components
    this.keepAlive = false // This applies to createServer and createClient calls below
    this.hideErrors = options.hideErrors ?? false

    if (!this.targetHost || !this.targetPort || !this.proxyPort || !this.version) {
      throw new Error('Missing required options: targetHost, targetPort, proxyPort, version')
    }

    this.proxyServer = null
    this.clients = new Map() // Map<clientId, ClientState>
  }

  // Starts the proxy server listening for incoming connections.
  listen () {
    if (this.proxyServer) {
      console.warn('[Proxy] Server already listening.')
      return
    }

    this.proxyServer = mc.createServer({
      'online-mode': this.onlineMode,
      port: this.proxyPort,
      version: this.version,
      keepAlive: this.keepAlive, // Use the class property
      hideErrors: this.hideErrors
    })

    this.proxyServer.on('login', (serverClient) => {
      this._handleNewClientConnection(serverClient)
    })

    this.proxyServer.on('error', (err) => {
      console.error('[ProxyServer Listen Error]', err)
      this.emit('error', err) // Emit error on the Proxy instance
    })

    this.proxyServer.on('listening', () => {
      console.log(`[ProxyServer] Listening on 127.0.0.1:${this.proxyPort}`)
      this.emit('listening')
    })
  }

  // Handles a new client connecting to the proxy server.
  _handleNewClientConnection (serverClient) {
    const clientId = serverClient.id ?? serverClient.username // Use ID or fallback to username
    console.log(`[ProxyServer] Client ${serverClient.username} (${serverClient.socket.remoteAddress}) connected to proxy. ID: ${clientId}`)

    const clientState = {
      serverClient,
      targetClient: null,
      world: new World(),
      playerPosition: new Vec3(0, 0, 0),
      yaw: 0,
      pitch: 0,
      onGround: false,
      ended: false
    }
    this.clients.set(clientId, clientState)

    const targetClient = mc.createClient({
      host: this.targetHost,
      port: this.targetPort,
      username: serverClient.username,
      version: serverClient.version,
      keepAlive: this.keepAlive, // Use the class property
      auth: 'offline', // Assuming offline for simplicity based on original script
      hideErrors: this.hideErrors
    })
    clientState.targetClient = targetClient

    console.log(`[Proxy] Attempting connection to target server ${this.targetHost}:${this.targetPort} for ${serverClient.username}...`)

    // --- Packet Forwarding & Handling ---

    // Client (Player) -> Proxy -> Target Server
    serverClient.on('packet', (data, meta) => {
      if (clientState.ended) return
      // Forward PLAY state packets from player to target server
      if (targetClient.state === states.PLAY && meta.state === states.PLAY) {
        // console.log(`[Proxy C->S] Forwarding ${meta.name}`);
        targetClient.write(meta.name, data)
        // Update internal player position state
        this._handleServerBoundPosition(clientState, meta.name, data)
      }
    })

    // Target Server -> Proxy -> Client (Player)
    targetClient.on('packet', (data, meta) => {
      if (clientState.ended) return
      // Forward PLAY state packets from target server to player
      if (serverClient.state === states.PLAY && meta.state === states.PLAY) {
        // console.log(`[Proxy S->C] Forwarding ${meta.name}`);
        try {
          // Update internal world state based on forwarded packets
          this._handleWorldPacket(clientState, meta.name, data)
          // Update internal player position state
          this._handleClientBoundPosition(clientState, meta.name, data)
        } catch (error) {
          console.error(`[Proxy Error ${clientId}] Failed to handle packet ${meta.name}:`, error)
          console.error(error.stack)
        }
        // Forward the packet to the actual player client
        serverClient.write(meta.name, data)
      }
    })

    // --- Connection End/Error Handling ---
    const endConnection = (side, reason) => {
      if (clientState.ended) return
      clientState.ended = true
      console.log(`[Proxy] Connection ended for client ${clientId} by ${side}. Reason: ${reason}`)
      // Use a slight delay to ensure packets might be flushed before ending sockets
      setTimeout(() => {
        serverClient.socket?.destroy() // Forcefully close sockets if end() doesn't work quickly
        targetClient.socket?.destroy()
      }, 100)
      serverClient.end(reason || 'Proxy connection ended.')
      targetClient.end(reason || 'Proxy connection ended.')
      this.clients.delete(clientId)
      this.emit('clientDisconnect', clientId, reason)
    }

    serverClient.on('end', (reason) => endConnection('Client', reason))
    targetClient.on('end', (reason) => endConnection('Target Server', reason))

    serverClient.on('error', (err) => {
      console.error(`[ProxyServer Client Error ${clientId}]`, err)
      endConnection('Client Error', err.message)
      this.emit('clientError', clientId, err)
    })
    targetClient.on('error', (err) => {
      console.error(`[TargetClient Error ${clientId}]`, err)
      endConnection('Target Server Error', err.message)
      this.emit('clientError', clientId, err)
    })

    // --- Initial Login Forwarding ---
    targetClient.on('login', (packet) => {
      console.log(`[Proxy] Client ${clientId} logged into target server.`)
      //this._handlePlayerPositionPacket(clientState, 'position', packet) // Initial position
      // Emit the login event *with* the client state object
      this.emit('clientLogin', clientId, clientState)
    })

    targetClient.on('connect', () => {
      console.log(`[Proxy] Client ${clientId} successfully connected to target server.`)
    })
  }

  // Processes world-related packets for a specific client's world state.
  _handleWorldPacket (clientState, packetName, packetData) {
    const { world } = clientState
    const clientId = clientState.serverClient.id ?? clientState.serverClient.username
    switch (packetName) {
      case 'map_chunk': {
        // console.log(`[World ${clientId}] map_chunk at ${packetData.x}, ${packetData.z}`);
        const dataBuffer = Buffer.isBuffer(packetData.chunkData) ? packetData.chunkData : Buffer.from(packetData.chunkData)
        try {
          world.loadColumn(packetData.x, packetData.z, dataBuffer)
        } catch (e) {
          console.error(`[World ${clientId}] Error loading chunk ${packetData.x},${packetData.z}:`, e)
        }
        break
      }
      case 'unload_chunk': {
        // console.log(`[World ${clientId}] unload_chunk at ${packetData.chunkX}, ${packetData.chunkZ}`);
        world.unloadColumn(packetData.chunkX, packetData.chunkZ)
        break
      }
      case 'block_change': {
        // console.log(`[World ${clientId}] block_change at ${packetData.location.x}, ${packetData.location.y}, ${packetData.location.z} to ${packetData.type}`);
        const pos = packetData.location
        const stateId = packetData.type
        try {
          world.setBlockStateId(pos.x, pos.y, pos.z, stateId)
        } catch (e) {
          console.error(`[World ${clientId}] Error setting block ${pos.x},${pos.y},${pos.z} to ${stateId}:`, e)
        }
        break
      }
      case 'multi_block_change': {
        // console.log(`[World ${clientId}] multi_block_change`);
        let chunkX, chunkZ, sectionY
        if (packetData.chunkCoordinates) { // 1.20.2+
          chunkX = packetData.chunkCoordinates.x
          chunkZ = packetData.chunkCoordinates.z
          sectionY = packetData.chunkCoordinates.y
        } else { // Older
          chunkX = packetData.chunkX
          chunkZ = packetData.chunkZ
          sectionY = -1 // Indicate we need to calculate based on record's Y
          // console.warn(`[Proxy ${clientId}] Handling legacy multi_block_change format, Y coordinate calculation might be needed if minY is not 0.`);
        }

        for (const record of packetData.records) {
          try {
            const recordBigInt = BigInt(record)
            const blockIndex = recordBigInt >> 12n
            const posData = recordBigInt & 0xfffn

            const relativeY = Number(posData & 0xfn)
            const z = Number((posData >> 4n) & 0xfn)
            const x = Number((posData >> 8n) & 0xfn)

            const worldX = chunkX * 16 + x
            // If sectionY was provided (1.20.2+), use it directly.
            // Otherwise (older versions), calculate based on relative Y. This assumes minY=0.
            // TODO: Make this robust for older versions if minY can change.
            // For 1.21.1, sectionY *should* be provided.
            const worldY = sectionY !== -1 ? (sectionY * 16 + relativeY) : relativeY
            const worldZ = chunkZ * 16 + z
            const newStateId = Number(blockIndex)

            world.setBlockStateId(worldX, worldY, worldZ, newStateId)
          } catch (e) {
            console.error(`[World ${clientId}] Error setting block state ID for multi block change record (${record}):`, e)
          }
        }
        break
      }
      // No default case needed, just ignore other packets
    }
  }

  // Handles position synchronization sent to the client
  _handleClientBoundPosition(clientState, packetName, packetData) {
    if (packetName === 'position' || packetName === 'position_look') {
      clientState.playerPosition.set(packetData.x, packetData.y, packetData.z);
    }
    
    if (packetName === 'position_look' || packetName === 'look') {
      clientState.yaw = packetData.yaw;
      clientState.pitch = packetData.pitch;
    }

  }

  // Handles client position information sent to the server
  _handleServerBoundPosition(clientState, packetName, packetData) {
    if (packetName === 'position' || packetName === 'position_look') {
      clientState.playerPosition.set(packetData.x, packetData.y, packetData.z);
      clientState.onGround = packetData.onGround
    }
    
    if (packetName === 'position_look' || packetName === 'look') {
      clientState.yaw = packetData.yaw;
      clientState.pitch = packetData.pitch;
    }
  }

  // Closes all client connections and stops the proxy server.
  close () {
    console.log('[Proxy] Closing proxy server...')
    for (const [clientId, clientState] of this.clients.entries()) {
      console.log(`[Proxy] Disconnecting client ${clientId}...`)
      clientState.serverClient?.end('Proxy shutting down.')
      clientState.targetClient?.end('Proxy shutting down.')
    }
    this.clients.clear()

    if (this.proxyServer) {
      this.proxyServer.close(() => {
        console.log('[Proxy] Server closed.')
        this.proxyServer = null
        this.emit('close')
      })
    } else {
      this.emit('close')
    }
  }

  // Gets the world instance for a specific connected client.
  getClientWorld (clientId) {
    return this.clients.get(clientId)?.world ?? null
  }

  // Gets the player position for a specific connected client.
  getClientPosition (clientId) {
    return this.clients.get(clientId)?.playerPosition ?? null
  }
}

module.exports = Proxy