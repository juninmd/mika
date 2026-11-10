import { EXPRESSION_ACTION_MAP, type Mood } from './live2d-animator-config'

const FOCUSED_EXPRESSIONS = new Set(['thinking', 'evil', 'serious', 'focus'])
const ENERGETIC_EXPRESSIONS = new Set([
  'happy',
  'excited',
  'love',
  'laugh',
  'panic',
  'six-seven',
  'floss',
  'orange-justice',
  'groove',
  'bounce',
  'clap',
])
const TIRED_EXPRESSIONS = new Set(['bored', 'doze', 'slump'])

export function getMoodForExpression(expression: string): Mood {
  if (FOCUSED_EXPRESSIONS.has(expression)) return 'focused'
  if (ENERGETIC_EXPRESSIONS.has(expression)) return 'energetic'
  if (TIRED_EXPRESSIONS.has(expression)) return 'tired'
  return 'neutral'
}

export function shouldLeanForwardForExpression(expression: string): boolean {
  return FOCUSED_EXPRESSIONS.has(expression)
}

export function getReactionActionNames(expression: string): string[] {
  return EXPRESSION_ACTION_MAP[expression] ?? EXPRESSION_ACTION_MAP.neutral
}
