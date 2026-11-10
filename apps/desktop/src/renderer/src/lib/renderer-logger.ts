const MAX_SERIALIZED_CHARS = 800

function safeSerialize(data: unknown): string {
  if (data instanceof Error) return `${data.name}: ${data.message}`
  if (typeof data !== 'object' || data === null) return String(data)

  const seen = new WeakSet<object>()
  let json: string
  try {
    json = JSON.stringify(data, (_key, value) => {
      if (value instanceof Error) return { name: value.name, message: value.message }
      if (typeof value === 'bigint') return value.toString()
      if (typeof value === 'function') return '[function]'
      if (typeof value === 'object' && value !== null) {
        if (seen.has(value)) return '[circular]'
        seen.add(value)
      }
      return value
    })
  } catch {
    return '{"serialization":"failed"}'
  }
  if (json === undefined) return ''
  return json.length > MAX_SERIALIZED_CHARS ? `${json.slice(0, MAX_SERIALIZED_CHARS)}…(truncado)` : json
}

function callerFrame(): string {
  const stack = new Error().stack
  if (!stack) return ''
  const lines = stack.split('\n')

  const frame = lines[4] ?? ''
  const match = frame.match(/([^/\\()\s]+\.[jt]sx?):(\d+):\d+/)
  return match ? `${match[1]}:${match[2]}` : ''
}

function format(message: string, data?: unknown): string {
  const at = callerFrame()
  const head = at ? `${message} (${at})` : message
  if (data === undefined) return head
  const serialized = safeSerialize(data)
  return serialized ? `${head} ${serialized}` : head
}

export const rendererLog = {
  debug(message: string, data?: unknown): void {
    console.debug(format(message, data))
  },
  info(message: string, data?: unknown): void {
    console.info(format(message, data))
  },
  warn(message: string, data?: unknown): void {
    console.warn(format(message, data))
  },
  error(message: string, data?: unknown): void {
    console.error(format(message, data))
  },
}
