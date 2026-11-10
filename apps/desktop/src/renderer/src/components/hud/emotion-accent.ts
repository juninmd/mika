/** Accent the speech bubble borrows from her current expression — a semantic token, never a literal. */
const GROUPS: ReadonlyArray<readonly [readonly string[], string]> = [
  [['happy', 'excited', 'laugh', 'wink', 'smug'], 'var(--ok)'],
  [['love', 'embarrassed'], 'var(--neural-coral)'],
  [['sad', 'bored', 'sleep'], 'var(--info)'],
  [['angry', 'evil', 'disgusted'], 'var(--danger)'],
  [['surprised', 'panic', 'confused'], 'var(--warn)'],
]

export function emotionAccent(expression: string | undefined): string {
  if (!expression) return 'var(--accent)'
  return GROUPS.find(([names]) => names.includes(expression))?.[1] ?? 'var(--accent)'
}
