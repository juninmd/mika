export type UiLogLevel = 'debug' | 'info' | 'warn' | 'error'

export interface UiLogEntry {
  level: UiLogLevel
  scope: string
  msg: string
  meta?: Record<string, unknown>
}

export interface UiLogger {
  debug(msg: string, meta?: Record<string, unknown>): void
  info(msg: string, meta?: Record<string, unknown>): void
  warn(msg: string, meta?: Record<string, unknown>): void
  error(msg: string, meta?: Record<string, unknown>): void
  child(scope: string): UiLogger
}

type UiLogBridge = (entry: UiLogEntry) => Promise<boolean>

const MAX_PENDING = 200

let bridgeState: 'unknown' | 'probing' | 'live' | 'absent' = 'unknown'
const pending: UiLogEntry[] = []

function readBridge(): UiLogBridge | null {
  const api = (globalThis as { electronAPI?: { runtime?: { log?: UiLogBridge } } }).electronAPI
  return typeof api?.runtime?.log === 'function' ? api.runtime.log : null
}

function writeToConsole(entry: UiLogEntry): void {
  const suffix = entry.meta && Object.keys(entry.meta).length > 0 ? ` ${JSON.stringify(entry.meta)}` : ''
  console[entry.level](`[${entry.scope}] ${entry.msg}${suffix}`)
}

function drain(bridge: UiLogBridge | null): void {
  const queued = pending.splice(0, pending.length)
  for (const entry of queued) {
    if (bridge) void bridge(entry)
    else writeToConsole(entry)
  }
}

function settle(bridge: UiLogBridge, alive: boolean, probe: UiLogEntry): void {
  bridgeState = alive ? 'live' : 'absent'
  // A live probe was already delivered; draining it again would log it twice.
  const index = alive ? pending.indexOf(probe) : -1
  if (index >= 0) pending.splice(index, 1)
  drain(alive ? bridge : null)
}

function emit(entry: UiLogEntry): void {
  if (bridgeState === 'live') {
    const bridge = readBridge()
    if (bridge) {
      void bridge(entry)
      return
    }
    bridgeState = 'absent'
  }

  if (bridgeState === 'absent') {
    writeToConsole(entry)
    return
  }

  if (pending.length < MAX_PENDING) pending.push(entry)

  if (bridgeState === 'probing') return

  const bridge = readBridge()
  if (!bridge) {
    bridgeState = 'absent'
    drain(null)
    return
  }

  bridgeState = 'probing'
  bridge(entry).then(
    (ok) => settle(bridge, ok !== false, entry),
    () => settle(bridge, false, entry),
  )
}

export function createUiLogger(scope: string): UiLogger {
  return {
    debug: (msg, meta) => emit({ level: 'debug', scope, msg, meta }),
    info: (msg, meta) => emit({ level: 'info', scope, msg, meta }),
    warn: (msg, meta) => emit({ level: 'warn', scope, msg, meta }),
    error: (msg, meta) => emit({ level: 'error', scope, msg, meta }),
    child: (childScope) => createUiLogger(`${scope}:${childScope}`),
  }
}
