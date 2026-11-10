import { type UsageMetadata, VTUBER_EXPRESSIONS, type VTuberExpression } from '@mika/sdk/contracts'
import type { SessionUsage } from '../components/app/app-shell.types'
import { useChatStore } from '../store/useChatStore'

export type UseMikaEventsProps = {
  enqueueTTS: (
    audio: string,
    mimeType: string,
    words?: Array<{ word: string; offset_ms: number; duration_ms: number }>,
  ) => void
  setExpression: (expr: VTuberExpression, intensity?: number) => void
  onTTSError?: (message: string) => void
  stopTTS?: () => void
}

const STREAM_FLUSH_INTERVAL_MS = 48
const STREAM_FLUSH_CHARS = 96

const BUBBLE_MIN_DWELL_MS = 4_000
const BUBBLE_MAX_DWELL_MS = 30_000

const BUBBLE_MS_PER_CHAR = 55

export function bubbleDwellMs(text: string): number {
  return Math.min(BUBBLE_MAX_DWELL_MS, Math.max(BUBBLE_MIN_DWELL_MS, text.length * BUBBLE_MS_PER_CHAR))
}

export function aggregateChunks() {
  let buffer = ''
  let flushTimer: ReturnType<typeof setTimeout> | null = null
  let flushCallback: ((text: string) => void) | null = null

  const flush = () => {
    if (flushTimer) {
      clearTimeout(flushTimer)
      flushTimer = null
    }
    if (buffer && flushCallback) {
      flushCallback(buffer)
      buffer = ''
    }
  }

  return {
    push: (chunk: string) => {
      buffer += chunk
      if (buffer.length >= STREAM_FLUSH_CHARS) {
        flush()
        return
      }
      flushTimer ??= setTimeout(flush, STREAM_FLUSH_INTERVAL_MS)
    },
    onFlush: (cb: (text: string) => void) => {
      flushCallback = cb
    },
    flush,
    reset: () => {
      buffer = ''
      if (flushTimer) clearTimeout(flushTimer)
      flushTimer = null
    },
  }
}

export function updateActiveTool({
  name,
  success,
  durationMs,
  finishedAt,
}: {
  name: string
  success?: boolean
  durationMs?: number
  finishedAt: number
}) {
  const currentActive = useChatStore.getState().activeTool
  useChatStore.getState().setActiveTool(
    currentActive && currentActive.name === name
      ? {
          ...currentActive,
          finishedAt,
          durationMs: durationMs ?? finishedAt - currentActive.startedAt,
          success: success ?? true,
        }
      : currentActive,
  )
}

export function accumulateSessionUsage(prev: SessionUsage, usage: UsageMetadata): SessionUsage {
  return {
    promptTokens: prev.promptTokens + usage.promptTokens,
    completionTokens: prev.completionTokens + usage.completionTokens,
    totalTokens: prev.totalTokens + usage.totalTokens,
    costUsd: prev.costUsd + (usage.costUsd ?? 0),
    turns: prev.turns + 1,

    lastPromptTokens: usage.promptTokens,

    contextWindow: usage.contextWindow ?? 0,

    unknownCostTurns: prev.unknownCostTurns + (usage.costUsd === undefined ? 1 : 0),
  }
}

// Mirrors the Rust regex in speech_text.rs that strips these for TTS. Emotion/action tags
// (`<|SMUG|>`, `<|TAG:0.5|>`, legacy `[TAG]`) drive the avatar through `leadingAvatarTag` (the
// opening one) and the `onAvatarExpression` event, so stripping them here is display-only cleanup.
// `[TAG]` stays uppercase-only and only the spaces right after a tag are eaten: replies carry code
// (`arr[i]`, indentation, blank lines) that a looser pattern would mangle.
// Hyphens: body actions are named `throw-tomato`, `six-seven`.
const ACTION_TAG_PATTERN = /(?:<\|[a-zA-Z][a-zA-Z0-9_-]*(?::[\d.]+)?\|>|\[[A-Z][A-Z0-9_-]*(?::[\d.]+)?\])[ \t]*/g

export const stripActionTags = (text: string) => text.replace(ACTION_TAG_PATTERN, '').trimStart()

// Streaming can split a tag across two chunks (`<|SM` + `UG|>`), so callers must run this over
// the full accumulated text on every flush, never per chunk in isolation.
export const sanitizeStreamBatch = stripActionTags

const KNOWN_EXPRESSIONS: ReadonlySet<string> = new Set(VTUBER_EXPRESSIONS)
const OPENING_TAG = /^\s*(?:<\|([a-zA-Z][\w-]*)(?::([\d.]+))?\|>|\[([A-Z][A-Z0-9_-]*)(?::([\d.]+))?\])/
/** A tag longer than this never closed: the reply simply did not open with one. */
const TAG_SCAN_LIMIT = 64

export interface AvatarTag {
  expression: VTuberExpression
  intensity: number
}

/** Could the rest still grow into a tag once more of the stream arrives? */
function mayStillBeTag(rest: string): boolean {
  const head = rest.trimStart()
  if (head === '') return true
  if (head.length > TAG_SCAN_LIMIT) return false
  return /^(?:<(?:\|[\w:.-]*)?|\[[A-Z0-9_:.-]*)$/.test(head)
}

/**
 * The avatar pose the reply opens with (`<|WAVE|> Oi!` → wave): system-tail.md makes the model start
 * every reply with one. Walks the opening run of tags and returns the first the avatar knows;
 * `'pending'` while a streamed tag may still be completing, `null` once the reply plainly has none.
 */
export function leadingAvatarTag(text: string): AvatarTag | 'pending' | null {
  let rest = text
  for (let match = OPENING_TAG.exec(rest); match; match = OPENING_TAG.exec(rest)) {
    const name = (match[1] ?? match[3]).toLowerCase().replaceAll('_', '-')
    if (KNOWN_EXPRESSIONS.has(name)) {
      const raw = Number(match[2] ?? match[4] ?? 1)
      return {
        expression: name as VTuberExpression,
        intensity: Number.isFinite(raw) ? Math.min(1, Math.max(0, raw)) : 1,
      }
    }
    rest = rest.slice(match[0].length)
  }
  return mayStillBeTag(rest) ? 'pending' : null
}
