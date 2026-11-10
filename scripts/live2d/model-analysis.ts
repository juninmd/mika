import { basename } from 'node:path'
import { listReferenceMotions, measureParameters, readDisplayInfo, readPhysicsOutputs } from './model-analysis-io'

export const ACTION_GROUP = 'Action'

export const MODELS_ROOT = ['apps', 'desktop', 'src', 'renderer', 'public', 'live2d']

export interface ParameterStats {
  min: number

  max: number

  curves: number

  base: number
}

export interface ModelAnalysis {
  name: string

  dir: string

  model3Path: string

  stem: string

  declaredParams: Set<string> | null

  displayNames: Map<string, string>

  physicsOutputs: Set<string>

  stats: Map<string, ParameterStats>

  referenceMotionCount: number
}

export interface Keyframe {
  time: number
  value: number
}

export function curveKeyframes(segments: number[]): Keyframe[] {
  const keys: Keyframe[] = [{ time: segments[0], value: segments[1] }]
  let i = 2
  while (i < segments.length) {
    if (segments[i] === 1) {
      keys.push({ time: segments[i + 5], value: segments[i + 6] })
      i += 7
    } else {
      keys.push({ time: segments[i + 1], value: segments[i + 2] })
      i += 3
    }
  }
  return keys
}

export { listModelDirs } from './model-analysis-io'

export function analyzeModel(model: { name: string; dir: string; model3Path: string }): ModelAnalysis {
  const { name, dir, model3Path } = model
  const stem = basename(model3Path).replace('.model3.json', '')

  const { declared: declaredParams, names: displayNames } = readDisplayInfo(dir)
  const physicsOutputs = readPhysicsOutputs(dir)
  const referenceFiles = listReferenceMotions(dir, model3Path)
  const stats = measureParameters(referenceFiles)

  return {
    name,
    dir,
    model3Path,
    stem,
    declaredParams,
    displayNames,
    physicsOutputs,
    stats,
    referenceMotionCount: referenceFiles.length,
  }
}
