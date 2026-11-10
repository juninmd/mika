import { dominantVowel, type VowelWeights } from '../../lib/lip-sync-vowels'
import { rendererLog } from '../../lib/renderer-logger'
import {
  listModelParameters,
  probeModelParameters,
  readParameterRanges,
  resolveLipSyncParams,
} from './live2d-animator-parameter-probe'
import type { AnimatorState } from './live2d-animator-state'
import { Live2DBodyRig } from './live2d-body-rig'
import type { Live2DModelInstance } from './live2d-loader'

const MOUTH_OPEN_PARAM_CANDIDATES = ['ParamMouthOpenY', 'PARAM_MOUTH_OPEN_Y', 'ParamMouthOpen', 'ParamMouthUp'] as const
const MOUTH_FORM_PARAM_CANDIDATES = ['ParamMouthForm', 'PARAM_MOUTH_FORM'] as const

const VOWEL_PARAM_CANDIDATES = {
  a: 'ParamA',
  i: 'ParamI',
  u: 'ParamU',
  e: 'ParamE',
  o: 'ParamO',
} as const

export function probeParameters(s: AnimatorState, model: Live2DModelInstance): void {
  s.supportedParams = probeModelParameters(model)

  const params = listModelParameters(model)
  const declared = resolveLipSyncParams(model).filter((id) => params.includes(id))
  s.mouthOpenParams =
    declared.length > 0 ? declared : MOUTH_OPEN_PARAM_CANDIDATES.filter((id) => params.includes(id)).slice(0, 1)
  s.mouthFormParam = MOUTH_FORM_PARAM_CANDIDATES.find((id) => params.includes(id)) ?? null

  s.vowelParams = {}
  for (const [vowel, id] of Object.entries(VOWEL_PARAM_CANDIDATES) as Array<[keyof VowelWeights, string]>) {
    if (params.includes(id)) s.vowelParams[vowel] = id
  }

  if (Object.keys(s.vowelParams).length < 3) s.vowelParams = {}

  const vowelIds = new Set(Object.values(s.vowelParams))
  if (vowelIds.size > 0) {
    s.mouthOpenParams = s.mouthOpenParams.filter((id) => !vowelIds.has(id))
  }

  const rig = new Live2DBodyRig(readParameterRanges(model))
  s.bodyRig = rig.isEmpty ? null : rig
  if (s.bodyRig) {
    rendererLog.debug('[Live2D] juntas do corpo resolvidas', rig.describe())
  } else {
    rendererLog.info('[Live2D] skin sem juntas de corpo conhecidas — braços ficam no repouso do rig')
  }

  rendererLog.debug('[Live2D] parâmetros do modelo', {
    total: params.length,
    declaradosNoLipSync: resolveLipSyncParams(model),
    vogais: Object.values(s.vowelParams),
    bocaEscolhida: s.mouthOpenParams,
    porFallback: declared.length === 0,
    formaEscolhida: s.mouthFormParam,
  })

  if (s.mouthOpenParams.length === 0 && vowelIds.size === 0) {
    rendererLog.warn('[Live2D] modelo sem parâmetro de boca — lip-sync não terá efeito', {
      total: params.length,
      candidatos: MOUTH_OPEN_PARAM_CANDIDATES,
      vogais: Object.keys(VOWEL_PARAM_CANDIDATES),
    })
  }
}

export function updateLipSync(s: AnimatorState, model: Live2DModelInstance, dt: number) {
  const smoothed = s.lipSync.update({
    mouthOpenness: s.mouthOpenness,
    volume: s.volumeRef?.current,
    vowels: s.vowelsRef?.current ?? null,
    vowelParams: s.vowelParams,
    expression: s.currentExpression,
    speaking: s.isSpeaking,
    time: s.time,
    dt,
    noise: s.noise,
    model,
    mouthFormParam: s.mouthFormParam,
  })

  traceLipSync(s, smoothed)

  if (s.isSpeaking && smoothed > 0.1) {
    const speakPhase = s.time * 4.5
    const sway = Math.sin(speakPhase) * smoothed * 1.5
    const nod = Math.abs(Math.sin(speakPhase * 0.7)) * smoothed * 0.8
    s.headX.target += sway
    s.headY.target += nod
  }
}

function traceLipSync(s: AnimatorState, smoothed: number): void {
  if (!s.isSpeaking) {
    s.lipSyncTracedAt = 0
    return
  }
  const now = s.time
  if (s.lipSyncTracedAt !== 0 && now - s.lipSyncTracedAt < 0.5) return
  s.lipSyncTracedAt = now

  rendererLog.debug('[LipSync] frame', {
    speaking: s.isSpeaking,
    volume: Number((s.volumeRef?.current ?? -1).toFixed(3)),
    temVolumeRef: s.volumeRef !== null,
    mouthOpenness: Number(s.mouthOpenness.toFixed(3)),
    smoothed: Number(smoothed.toFixed(3)),
    expression: s.currentExpression,

    paramBoca: s.mouthOpenParams,

    ...(Object.keys(s.vowelParams).length > 0 && s.vowelsRef
      ? { vogal: dominantVowel(s.vowelsRef.current).vowel }
      : {}),
  })
}
