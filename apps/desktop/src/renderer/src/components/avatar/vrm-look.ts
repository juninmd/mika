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
const flat = (material: RegExp, color: string, shade: string) => ({ material, color, shade, flat: true })
const HAIR = /^Hair/i
const IRIS = /^EyeIris/i
const TOPS = /^Tops/i
const BOTTOMS = /^Bottoms/i
const SHOES = /^Shoes/i

// One VRM skin per existing Live2D model, colors read from each preview.png. Color only — blazer,
// scarf, hat and curl need authored VRoid geometry.
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
  {
    id: 'hiyori',
    name: 'Hiyori',
    tints: [
      flat(HAIR, '#9a7a5c', '#4e3a2a'),
      tint(IRIS, '#8ea0c4', '#4a5c86'),
      tint(TOPS, '#f3dfb6', '#b89a6a'),
      tint(BOTTOMS, '#33405e', '#161c2e'),
      tint(SHOES, '#5a4030', '#2a1c12'),
    ],
  },
  {
    id: 'mao',
    name: 'Mao',
    tints: [
      flat(HAIR, '#f4a94e', '#a86a22'),
      tint(IRIS, '#6db0f0', '#2f6fbf'),
      tint(TOPS, '#f6f6f8', '#b8b8c8'),
      tint(BOTTOMS, '#2d3266', '#151834'),
      tint(SHOES, '#2a2f5c', '#12152c'),
    ],
  },
  {
    id: 'mark',
    name: 'Mark',
    tints: [
      flat(HAIR, '#9a7048', '#4e3620'),
      tint(IRIS, '#5ad46a', '#1f8a2c'),
      tint(TOPS, '#ff5c5c', '#b52e2e'),
      flat(BOTTOMS, '#3c72d6', '#1c3f8a'),
      flat(SHOES, '#f2f2f2', '#9a9a9a'),
    ],
  },
  {
    id: 'rice',
    name: 'Rice',
    tints: [
      flat(HAIR, '#efe7d8', '#b8ad98'),
      tint(IRIS, '#5ad8c0', '#1f8f7c'),
      tint(TOPS, '#6d84a6', '#38496a'),
      flat(BOTTOMS, '#f2f0ea', '#b5b2a8'),
      tint(SHOES, '#2b2b33', '#121216'),
    ],
  },
]

const HARU_BLAZER = /^Haru_Blazer$/
const HARU_PIPING = /^Haru_Piping$/

export const HARU_OUTFITS: readonly VrmLook[] = [
  {
    id: 'seifuku',
    name: 'Seifuku',
    tints: [tint(HARU_BLAZER, '#f6f7fb', '#b3bdd3'), tint(HARU_PIPING, '#2c3a66', '#161d3a')],
    show: /^Outfit_Seifuku_/,
    hide: /^Haru_(Lapel|Pocket|Button|CenterSeam|Shirt|FoldedCollar|Scarf|Skirt|Seam|Tights)/,
  },
  {
    id: 'maid',
    name: 'Maid',
    tints: [tint(HARU_BLAZER, '#2b2735', '#121019'), tint(HARU_PIPING, '#fbfaff', '#c4c3d6')],
    show: /^Outfit_Maid_/,
    hide: /^Haru_(Lapel|Pocket|Button|CenterSeam|Shirt|Scarf|Skirt|Seam|Tights)/,
  },
  {
    id: 'idol',
    name: 'Idol',
    tints: [tint(HARU_BLAZER, '#ffb3d1', '#d97aa6'), tint(/^Haru_Stitch$/, '#ffb3d1', '#d97aa6')],
    show: /^Outfit_Idol_/,
    hide: /^Haru_(Scarf|Skirt|Seam|Button|Pocket|Tights)/,
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
