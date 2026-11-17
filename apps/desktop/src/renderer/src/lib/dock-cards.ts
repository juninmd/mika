/** A tool result worth floating beside the edge dock: what she just did, at a glance. */
export type DockCardContent = {
  kind: 'reminder' | 'hint' | 'recap' | 'daily'
  subject: string
  body: string
  /** Help-ladder step of a hint (`gamer::game_knowledge_tool::HelpLadder`). */
  level?: 'hint' | 'tip' | 'solution'
  /** When a reminder fires (epoch ms). */
  at?: number
  /** "What did we do today" (`game_session` today); `passenger` means the recaps were left out. */
  daily?: { sessions: number; deaths: number; streak: number; achievements: string[]; passenger: boolean }
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
const count = (value: unknown): number => (typeof value === 'number' ? value : 0)
const list = (value: unknown): Data[] =>
  Array.isArray(value) ? value.filter((item): item is Data => typeof item === 'object' && item !== null) : []

function dailyCard(data: Data): DockCardContent {
  const games = list(data.byGame)
    .map((g) => text(g.game))
    .filter(Boolean)
  return {
    kind: 'daily',
    subject: [text(data.totalDuration), games.join(', ')].filter(Boolean).join(' · '),
    body: text(list(data.highlights)[0]?.summary),
    daily: {
      sessions: count(data.sessionCount),
      deaths: count(data.deaths),
      streak: count(data.currentStreakDays),
      // Unique and non-empty: the card keys its rows by text.
      achievements: [...new Set(list(data.achievements).map((a) => text(a.description)))].filter(Boolean),
      passenger: data.passengerMode === true,
    },
  }
}

const LEVELS = new Set(['hint', 'tip', 'solution'])

/**
 * Card for a finished tool call, or `null`. Four results are worth it: a reminder just created
 * (`reminder`), a game guide (`game_knowledge`, with its help-ladder step) and a narrated session
 * (`game_session` describe) or the day summary (`game_session` today). Everything else already shows up as her reply.
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
  if (name === 'game_session' && text(data.day)) return dailyCard(data)
  if (name === 'game_session' && text(data.commentary))
    return { kind: 'recap', subject: text(data.gameName), body: text(data.commentary) }
  return null
}
