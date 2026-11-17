/** A tool result worth floating beside the edge dock: what she just did, at a glance. */
export type DockCardContent = {
  kind: 'reminder' | 'hint' | 'recap' | 'media'
  subject: string
  body: string
  /** Help-ladder step of a hint (`gamer::game_knowledge_tool::HelpLadder`). */
  level?: 'hint' | 'tip' | 'solution'
  /** When a reminder fires (epoch ms). */
  at?: number
  /** What a media reaction is about (`vision::media_reaction`). */
  media?: 'video' | 'live' | 'music'
}

type Data = Record<string, unknown>

function successData(result: unknown): Data | null {
  let value = result
  if (typeof value === 'string') {
    try {
      value = JSON.parse(value)
    } catch {
      return null
    }
  }
  const envelope = value as { success?: unknown; data?: unknown } | null
  if (envelope?.success !== true || typeof envelope.data !== 'object' || !envelope.data) return null
  return envelope.data as Data
}

const text = (value: unknown): string => (typeof value === 'string' ? value.trim() : '')
const LEVELS = new Set(['hint', 'tip', 'solution'])
const MEDIA = new Set(['video', 'live', 'music'])

/**
 * Card for a finished tool call, or `null`. Three results are worth it: a reminder just created
 * (`reminder`), a game guide (`game_knowledge`, with its help-ladder step) and a narrated session
 * (`game_session` describe). Everything else already shows up as her reply.
 */
export function dockCard(name: string, result: unknown): DockCardContent | null {
  const data = successData(result)
  if (!data) return null
  if (name === 'reminder' && text(data.subject) && typeof data.dueAt === 'number')
    return { kind: 'reminder', subject: text(data.subject), body: '', at: data.dueAt }
  if (name === 'game_knowledge' && text(data.guide)) {
    const level = LEVELS.has(text(data.helpLevel)) ? (text(data.helpLevel) as DockCardContent['level']) : undefined
    return { kind: 'hint', subject: text(data.game), body: text(data.guide), ...(level && { level }) }
  }
  if (name === 'game_session' && text(data.commentary))
    return { kind: 'recap', subject: text(data.gameName), body: text(data.commentary) }
  return null
}

/** Card for a real-time reaction (`source: 'media-reaction'` proactive thought), or `null`. */
export function mediaCard(payload: unknown): DockCardContent | null {
  const event = payload as { source?: unknown; media?: { kind?: unknown; title?: unknown } } | null
  if (event?.source !== 'media-reaction') return null
  const kind = text(event.media?.kind)
  const title = text(event.media?.title)
  if (!MEDIA.has(kind) || !title) return null
  return { kind: 'media', media: kind as DockCardContent['media'], subject: title, body: '' }
}
