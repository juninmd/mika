import { redactArgs } from './redaction'

export function typedEntries<T extends Record<string, unknown>>(obj: T): [keyof T, T[keyof T]][] {
  return Object.entries(obj) as [keyof T, T[keyof T]][]
}

export function setPath(obj: Record<string, unknown>, keys: string[], value: unknown): Record<string, unknown> {
  return writePathImmutable(obj, keys, value)
}

const OPAQUE = '[object Object]'
const MAX_ERROR_MESSAGE_LENGTH = 400

function isUsefulMessage(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0 && value !== OPAQUE
}

function detailsFrom(error: Record<string, unknown>): string | undefined {
  const nested = error.error
  if (isUsefulMessage(nested)) return nested
  if (nested && typeof nested === 'object' && isUsefulMessage((nested as Record<string, unknown>).message)) {
    return (nested as Record<string, unknown>).message as string
  }

  for (const key of ['responseBody', 'data', 'detail', 'body']) {
    const value = error[key]
    if (isUsefulMessage(value)) return value
    if (value && typeof value === 'object') {
      const stringified = safeStringify(value)
      if (stringified) return stringified
    }
  }

  const status = error.statusCode ?? error.status
  return typeof status === 'number' ? `HTTP ${status}` : undefined
}

function safeStringify(value: object): string | undefined {
  try {
    const text = JSON.stringify(redactArgs(value as Record<string, unknown>, false))
    if (!text || text === '{}' || text === 'null') return undefined
    return text.length > MAX_ERROR_MESSAGE_LENGTH ? `${text.slice(0, MAX_ERROR_MESSAGE_LENGTH)}…` : text
  } catch {
    return undefined
  }
}

export function toErrorMessage(error: unknown): string {
  if (error === null || error === undefined) return String(error)

  if (typeof error !== 'object') return String(error)

  const record = error as Record<string, unknown>
  if (isUsefulMessage(record.message)) return record.message

  const details = detailsFrom(record)
  if (details) return details

  const cause = record.cause
  if (cause !== undefined && cause !== null) {
    const fromCause = toErrorMessage(cause)
    if (isUsefulMessage(fromCause)) return fromCause
  }

  const stringified = safeStringify(error)
  if (stringified) return stringified

  const name = error.constructor?.name
  return isUsefulMessage(name) ? `${name} (sem mensagem)` : 'erro desconhecido (sem mensagem)'
}

export function readPath(obj: Record<string, unknown>, keys: string[]): unknown {
  let cur: unknown = obj
  for (const key of keys) {
    if (cur === null || cur === undefined || typeof cur !== 'object') return undefined
    cur = (cur as Record<string, unknown>)[key]
  }
  return cur
}

const DANGEROUS_KEYS = new Set(['__proto__', 'constructor', 'prototype'])

export function writePathImmutable(
  obj: Record<string, unknown>,
  keys: string[],
  value: unknown,
): Record<string, unknown> {
  if (keys.length === 0) return obj
  if (keys.some((key) => DANGEROUS_KEYS.has(key))) {
    throw new Error(`writePathImmutable: chave perigosa recusada (${keys.join('.')})`)
  }
  const root = { ...obj }
  let cur = root
  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i]
    const existing = cur[key]

    if (Array.isArray(existing)) {
      cur[key] = [...existing]
    } else {
      cur[key] = typeof existing === 'object' && existing !== null ? { ...(existing as Record<string, unknown>) } : {}
    }
    cur = cur[key] as Record<string, unknown>
  }
  const last = keys[keys.length - 1]
  if (last) {
    if (value === undefined) {
      delete cur[last]
    } else {
      cur[last] = value
    }
  }
  return root
}
