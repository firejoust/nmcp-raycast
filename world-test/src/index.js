const Proxy = require('./proxy') // Assuming proxy.js is in the same directory
const { Vec3 } = require('vec3') // Vec3 is needed here now

// --- Configuration ---
const TARGET_SERVER_HOST = process.argv[2] || 'localhost'
const TARGET_SERVER_PORT = parseInt(process.argv[3] || '25565', 10)
const PROXY_PORT = parseInt(process.argv[4] || '25566', 10)
const MC_VERSION = process.argv[5] || '1.21.1' // Use a version supported by prismarine-world-lite

if (!TARGET_SERVER_HOST) {
  console.error('Usage: node example.js <target_host> [target_port] [proxy_port] [mc_version]')
  process.exit(1)
}

// --- Create and Start Proxy ---
const proxy = new Proxy({
  targetHost: TARGET_SERVER_HOST,
  targetPort: TARGET_SERVER_PORT,
  proxyPort: PROXY_PORT,
  version: MC_VERSION
})

// Store intervals for cleanup
const clientIntervals = new Map()

proxy.on('listening', () => {
  console.log('Proxy is ready for connections.')
})

// The 'clientLogin' event now passes the clientState object
proxy.on('clientLogin', (clientId, clientState) => {
  console.log(`Client with ID ${clientId} has logged in through the proxy.`)

  // Access world and position directly from the provided clientState
  const { world, playerPosition } = clientState

  // Example: Periodically check the block below the player
  const intervalId = setInterval(() => {
    // Check if the client is still connected before accessing state
    if (clientState.ended) {
      clearInterval(intervalId)
      clientIntervals.delete(clientId) // Clean up map entry
      return
    }

    const posBelow = playerPosition.floored().offset(0, -1, 0)

    try {
      // Use the world instance specific to this client
      const blockStateId = world.getBlockStateId(posBelow.x, posBelow.y, posBelow.z)
      console.log(`[Proxy ${clientId}] Block below player at ${posBelow}: State ID ${blockStateId}`)

      // Example: Get the full block info
      // const blockInfo = world.getBlock(posBelow.x, posBelow.y, posBelow.z);
      // if (blockInfo) {
      //   console.log(`[Proxy ${clientId}] Block info below:`, blockInfo);
      // } else {
      //   console.log(`[Proxy ${clientId}] No block info below (chunk likely unloaded).`);
      // }
    } catch (e) {
      // console.warn(`[Proxy ${clientId}] Error getting block below player: ${e.message}`);
    }
  }, 5000) // Check every 5 seconds

  // Store the interval ID so we can clear it on disconnect
  clientIntervals.set(clientId, intervalId)
})

proxy.on('clientDisconnect', (clientId, reason) => {
  console.log(`Client ${clientId} disconnected. Reason: ${reason}`)
  // Clean up the interval associated with this client
  const intervalId = clientIntervals.get(clientId)
  if (intervalId) {
    clearInterval(intervalId)
    clientIntervals.delete(clientId)
    console.log(`[Proxy ${clientId}] Stopped block checking interval.`)
  }
})

proxy.on('clientError', (clientId, err) => {
  console.error(`[Proxy] Error for client ${clientId}:`, err)
})

proxy.on('error', (err) => {
  console.error('[Proxy] General server error:', err)
})

proxy.on('close', () => {
  console.log('[Proxy] Server has been closed.')
  // Clear any remaining intervals on full proxy shutdown
  for (const intervalId of clientIntervals.values()) {
    clearInterval(intervalId)
  }
  clientIntervals.clear()
})

// Start listening
proxy.listen()

// --- Graceful Shutdown ---
function shutdown () {
  console.log('Shutting down proxy...')
  proxy.close() // This will disconnect clients and stop the server
  // Allow some time for cleanup before exiting forcefully
  setTimeout(() => process.exit(0), 1000)
}

process.on('SIGINT', () => {
  console.log('SIGINT received.')
  shutdown()
})

process.on('SIGTERM', () => {
  console.log('SIGTERM received.')
  shutdown()
})