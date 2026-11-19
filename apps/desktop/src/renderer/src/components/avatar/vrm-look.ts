import { isVrmBodyAction } from './vrm-body-rig'

export function isVrmModelPath(path: string | null | undefined): boolean {
  return typeof path === 'string' && /\.vrm(#.*)?$/i.test(path.trim())
}

export const EXPRESSION_PRESET: Record<string, string> = {
  happy: 'happy',
  laugh: 'happy',
  excited: 'happy',
  love: 'happy',
  wink: 'happy',
  smug: 'happy',
  angry: 'angry',
  evil: 'angry',
  serious: 'angry',
  disgusted: 'angry',
  determined: 'angry',
  sad: 'sad',
  bored: 'relaxed',
  tired: 'relaxed',
  sleep: 'relaxed',
  surprised: 'surprised',
  panic: 'surprised',
  scared: 'surprised',
  shy: 'happy',
  embarrassed: 'happy',
  confused: 'surprised',
  thinking: 'neutral',
  neutral: 'neutral',
}

export const VRM_PRESETS = ['happy', 'angry', 'sad', 'relaxed', 'surprised'] as const

export const VOWEL_PRESET = { a: 'aa', i: 'ih', u: 'ou', e: 'ee', o: 'oh' } as const

// Live2D-generated actions mapped to an equivalent VRM expression pulse until VRM has its own motions.
export const ACTION_EXPRESSION: Record<string, string> = {
  happy: 'happy',
  cheer: 'happy',
  laugh: 'happy',
  wink: 'neutral',
  shy: 'happy',
  angry: 'angry',
  sad: 'sad',
  sleepy: 'relaxed',
  surprised: 'surprised',
  pout: 'angry',
  thinking: 'neutral',
  nod: 'neutral',
  shakehead: 'neutral',
  wave: 'happy',
  shrug: 'neutral',
  bow: 'relaxed',
  stretch: 'relaxed',
  presenting: 'happy',
  peace: 'happy',
  '22': 'happy',
  heart: 'happy',
  victory: 'happy',
  facepalm: 'angry',
  smug: 'happy',
  sigh: 'relaxed',
  beckon: 'happy',
  curious: 'neutral',
  salute: 'happy',
  whisper: 'neutral',
  celebrate: 'happy',
  relaxed: 'relaxed',
  point: 'happy',
  reading: 'neutral',
  sitting: 'relaxed',
  'six-seven': 'happy',
  floss: 'happy',
  'orange-justice': 'happy',
  yawn: 'relaxed',
  drink: 'relaxed',
  'throw-tomato': 'happy',
  senbonzakura: 'relaxed',
  clap: 'happy',
  'thumbs-up': 'happy',
  headshake: 'neutral',
  bounce: 'happy',
  slump: 'sad',
  focus: 'neutral',
  groove: 'happy',
  doze: 'relaxed',
  'fist-pump': 'happy',
  gasp: 'surprised',
  rage: 'angry',
  tada: 'happy',
}

// Peak weight of an action's face preset; `happy` above ~0.6 closes the eyes into the anime ^^ smile.
const ACTION_FACE_WEIGHT: Record<string, number> = {
  laugh: 0.85,
  surprised: 0.85,
  cheer: 0.75,
  celebrate: 0.75,
  victory: 0.7,
  pout: 0.9,
  facepalm: 0.9,
  sigh: 0.65,
  shy: 0.55,
  bounce: 0.75,
  clap: 0.7,
  'fist-pump': 0.7,
  tada: 0.75,
  slump: 0.8,
  rage: 0.9,
  doze: 0.8,
  gasp: 0.6,
}

export function actionFaceWeight(action: string): number {
  return ACTION_FACE_WEIGHT[action] ?? 0.55
}

export function expressionPreset(name: string): string {
  return EXPRESSION_PRESET[name] ?? ACTION_EXPRESSION[name] ?? 'neutral'
}

export const VRM_EXPRESSION_ACTIONS: Record<string, readonly string[]> = {
  thinking: ['thinking'],
  laugh: ['laugh'],
  shy: ['shy'],
  embarrassed: ['shy'],
  excited: ['cheer', 'victory', 'bounce'],
  happy: ['wave'],
  love: ['heart', 'shy'],
  determined: ['nod'],
  confused: ['shrug'],
  smug: ['peace'],
  angry: ['pout', 'rage'],
  disgusted: ['facepalm', 'headshake'],
  sad: ['sigh', 'slump'],
  tired: ['yawn', 'sigh', 'doze'],
  sleep: ['yawn', 'doze'],
  sleepy: ['yawn', 'doze'],
  bored: ['sigh', 'doze'],
  surprised: ['surprised'],
  panic: ['surprised', 'gasp'],
  scared: ['surprised', 'gasp'],
  beckon: ['beckon'],
  curious: ['curious'],
  salute: ['salute'],
  whisper: ['whisper'],
  celebrate: ['celebrate'],
  relaxed: ['relaxed'],
  reading: ['reading'],
  sitting: ['sitting'],
}

export function bodyActionForExpression(expression: string): string | null {
  const actions = VRM_EXPRESSION_ACTIONS[expression]
  if (actions && actions.length > 0) return actions[Math.floor(Math.random() * actions.length)]
  // Poses named after the action itself ('wave', 'bow', 'peace'…) reach the tool as expressions.
  if (expression === 'nod' || isVrmBodyAction(expression)) return expression
  return null
}

export interface VrmLook {
  id: string
  name: string
  // Tint by MToon material name prefix; empty = model as shipped. `flat` drops the base texture since
  // tint only multiplies — light colors over a dark texture would otherwise render flat regardless.
  tints: Array<{ material: RegExp; color: string; shade?: string; flat?: boolean }>
  // Outfits authored inside haru.vrm: `show` reveals their hidden `Outfit_*` meshes, `hide` drops
  // the blazer pieces the outfit replaces.
  show?: RegExp
  hide?: RegExp
}

const tint = (material: RegExp, color: string, shade: string) => ({ material, color, shade })
const HAIR = /^Hair/i
const IRIS = /^EyeIris/i
const TOPS = /^Tops/i
const BOTTOMS = /^Bottoms/i
const SHOES = /^Shoes/i

// Haru resolves to its own haru.vrm (see vrmModelPath); the entry keeps the saved `#look=haru` id valid.
export const VRM_LOOKS: readonly VrmLook[] = [
  { id: 'original', name: 'Mika Aurora', tints: [] },
  {
    id: 'haru',
    name: 'Haru',
    tints: [
      tint(HAIR, '#4a3a63', '#211a30'),
      tint(IRIS, '#8ccaff', '#3d86d6'),
      tint(TOPS, '#2a2a33', '#141419'),
      tint(BOTTOMS, '#2f2f3a', '#16161c'),
      tint(SHOES, '#26262e', '#111116'),
    ],
  },
]

const HARU_BLAZER = /^Haru_Blazer$/
const HARU_PIPING = /^Haru_Piping$/

export const HARU_OUTFITS: readonly VrmLook[] = [
  {
    id: 'maid',
    name: 'Maid',
    tints: [tint(HARU_BLAZER, '#2b2735', '#121019'), tint(HARU_PIPING, '#fbfaff', '#c4c3d6')],
    show: /^Outfit_Maid_/,
    hide: /^Haru_(Lapel|Pocket|Button|CenterSeam|Shirt|Scarf|Skirt|Seam|Tights)/,
  },
]

export const ORIGINAL_LOOK = VRM_LOOKS[0] as VrmLook
export function lookById(id: string | undefined): VrmLook {
  return [...VRM_LOOKS, ...HARU_OUTFITS].find((look) => look.id === id) ?? ORIGINAL_LOOK
}

const isOutfit = (lookId: string) => HARU_OUTFITS.some((look) => look.id === lookId)

export function meshVisibleInLook(meshName: string, look: VrmLook): boolean {
  if (/^Outfit_/.test(meshName)) return look.show?.test(meshName) ?? false
  return !look.hide?.test(meshName)
}

export function parseVrmModelPath(path: string): { url: string; look: VrmLook } {
  const [url = '', fragment = ''] = path.trim().split('#')
  const id = new URLSearchParams(fragment).get('look') ?? undefined
  // Already-saved configs for the old skin also get the redesigned model.
  if (id === 'haru' && url.endsWith('vrm/mika/mika.vrm')) {
    return { url: url.replace(/mika\.vrm$/, 'haru.vrm'), look: ORIGINAL_LOOK }
  }
  return { url, look: lookById(id) }
}

export function vrmModelPath(url: string, lookId: string): string {
  if (lookId === 'haru' && url.endsWith('vrm/mika/mika.vrm')) return url.replace(/mika\.vrm$/, 'haru.vrm')
  if (isOutfit(lookId)) return `${url.replace(/mika\.vrm$/, 'haru.vrm')}#look=${lookId}`
  return lookId === ORIGINAL_LOOK.id ? url : `${url}#look=${lookId}`
}

export function tintFor(materialName: string, look: VrmLook): { color: string; shade?: string; flat?: boolean } | null {
  return look.tints.find((tint) => tint.material.test(materialName)) ?? null
}
