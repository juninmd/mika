export type Mood = 'neutral' | 'energetic' | 'tired' | 'focused'

export const IDLE_MOTION_MIN_INTERVAL = 4_000
export const IDLE_MOTION_MAX_INTERVAL = 12_000

export const BREATH_FREQ_PRIMARY = 0.33
export const BREATH_FREQ_SECONDARY = 0.1

export const BLINK_INTERVAL_MIN = 2.0
export const BLINK_INTERVAL_MAX = 6.0
export const DOUBLE_BLINK_CHANCE = 0.2

export const EXPRESSION_ACTION_MAP: Record<string, string[]> = {
  happy: ['happy'],
  sad: ['sad'],
  surprised: ['surprised'],
  angry: ['angry'],
  thinking: ['thinking'],
  neutral: [],

  excited: ['cheer', 'happy'],
  confused: ['thinking', 'shakehead'],
  embarrassed: ['shy', 'pout'],
  serious: ['angry'],
  wink: ['wink'],
  smug: ['wink'],
  love: ['shy', 'happy'],
  evil: ['angry'],
  panic: ['surprised'],
  bored: ['sleepy', 'sad'],
  laugh: ['laugh', 'happy', 'cheer'],
  disgusted: ['angry', 'shakehead'],
  scared: ['surprised'],
  determined: ['nod'],
  tired: ['sleepy', 'sad'],
  shy: ['shy'],
  sleep: ['sleepy'],

  // Poses de corpo: o Live2D só tem as 15 motions de `scripts/live2d/action-defs.ts`, então cada uma
  // cai na mais próxima. Sem entrada aqui a tool responderia sucesso e o avatar não moveria um pixel.
  sitting: ['sleepy'],
  reading: ['thinking'],
  curious: ['thinking', 'shakehead'],
  salute: ['wave'],
  whisper: ['shy'],
  celebrate: ['cheer', 'happy'],
  relaxed: ['happy'],
  pout: ['pout'],
  facepalm: ['shakehead', 'sad'],
  sigh: ['sad'],
  beckon: ['wave'],
  wave: ['wave'],
  bow: ['nod'],
  stretch: ['sleepy'],
  peace: ['wink', 'happy'],
  '22': ['cheer', 'happy'],
  heart: ['shy', 'happy'],
  victory: ['cheer'],
  nod: ['nod'],
  shrug: ['shrug'],
  point: ['wave', 'nod'],
  'six-seven': ['wave', 'shrug'],
  floss: ['cheer', 'wave'],
  'orange-justice': ['cheer', 'shrug'],
  yawn: ['sleepy'],
  drink: ['nod'],
  'throw-tomato': ['cheer'],
  senbonzakura: ['cheer'],
  clap: ['cheer', 'happy'],
  'thumbs-up': ['nod', 'happy'],
  headshake: ['shakehead'],
  bounce: ['cheer'],
  slump: ['sad'],
  focus: ['thinking'],
  groove: ['cheer', 'wave'],
  doze: ['sleepy'],
  'fist-pump': ['cheer'],
  gasp: ['shy', 'surprised'],
  rage: ['angry'],
  tada: ['cheer', 'happy'],
}

export function randomBetween(min: number, max: number): number {
  return Math.random() * (max - min) + min
}
