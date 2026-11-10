import { existsSync, readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'
import { ACTION_GROUP, curveKeyframes, MODELS_ROOT, type ParameterStats } from './model-analysis'

interface Model3Json {
  FileReferences?: {
    Motions?: Record<string, Array<{ File: string; Name?: string; FadeInTime?: number; FadeOutTime?: number }>>
  }
  [key: string]: unknown
}

function readJson<T>(path: string): T {
  return JSON.parse(readFileSync(path, 'utf8')) as T
}

function readdirSorted(dir: string): string[] {
  return readdirSync(dir).sort()
}

function readdirSortedEntries(dir: string) {
  return readdirSync(dir, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
}

export function findFileBySuffix(dir: string, suffix: string): string | null {
  const match = readdirSorted(dir).find((entry) => entry.endsWith(suffix))
  return match ? join(dir, match) : null
}

function collectMotionFiles(dir: string, out: string[]): void {
  for (const entry of readdirSortedEntries(dir)) {
    const full = join(dir, entry.name)
    if (entry.isDirectory()) collectMotionFiles(full, out)
    else if (entry.name.endsWith('.motion3.json')) out.push(full)
  }
}

export function listModelDirs(root: string): Array<{ name: string; dir: string; model3Path: string }> {
  const base = join(root, ...MODELS_ROOT)
  if (!existsSync(base)) return []
  const found: Array<{ name: string; dir: string; model3Path: string }> = []
  for (const entry of readdirSortedEntries(base)) {
    if (!entry.isDirectory()) continue
    const dir = join(base, entry.name)
    const model3Path = findFileBySuffix(dir, '.model3.json')
    if (model3Path) found.push({ name: entry.name, dir, model3Path })
  }
  return found
}

export function readDisplayInfo(dir: string): { declared: Set<string> | null; names: Map<string, string> } {
  const names = new Map<string, string>()
  const cdiPath = findFileBySuffix(dir, '.cdi3.json')
  if (!cdiPath) return { declared: null, names }

  const declared = new Set<string>()
  const cdi = readJson<{ Parameters?: Array<{ Id: string; Name?: string }> }>(cdiPath)
  for (const param of cdi.Parameters ?? []) {
    declared.add(param.Id)
    if (param.Name) names.set(param.Id, param.Name)
  }
  return { declared, names }
}

export function readPhysicsOutputs(dir: string): Set<string> {
  const outputs = new Set<string>()
  const physicsPath = findFileBySuffix(dir, '.physics3.json')
  if (!physicsPath) return outputs

  const physics = readJson<{ PhysicsSettings?: Array<{ Output?: Array<{ Destination?: { Id?: string } }> }> }>(
    physicsPath,
  )
  for (const setting of physics.PhysicsSettings ?? []) {
    for (const output of setting.Output ?? []) {
      if (output.Destination?.Id) outputs.add(output.Destination.Id)
    }
  }
  return outputs
}

export function listReferenceMotions(dir: string, model3Path: string): string[] {
  const model3 = readJson<Model3Json>(model3Path)
  const generated = new Set(
    (model3.FileReferences?.Motions?.[ACTION_GROUP] ?? []).map((entry) => join(dir, entry.File).toLowerCase()),
  )
  const motionFiles: string[] = []
  collectMotionFiles(dir, motionFiles)
  return motionFiles.filter((file) => !generated.has(file.toLowerCase()))
}

function mostFrequent(counter: Map<number, number>): number {
  let best = 0
  let bestCount = -1
  for (const [value, count] of counter) {
    if (count > bestCount) {
      best = value
      bestCount = count
    }
  }
  return best
}

export function measureParameters(files: string[]): Map<string, ParameterStats> {
  const ranges = new Map<string, { min: number; max: number; curves: number }>()
  const firstValues = new Map<string, Map<number, number>>()

  for (const file of files) {
    const data = readJson<{ Curves?: Array<{ Target: string; Id: string; Segments: number[] }> }>(file)
    for (const curve of data.Curves ?? []) {
      if (curve.Target !== 'Parameter') continue
      const keys = curveKeyframes(curve.Segments)
      const values = keys.map((key) => key.value)
      const current = ranges.get(curve.Id) ?? {
        min: Number.POSITIVE_INFINITY,
        max: Number.NEGATIVE_INFINITY,
        curves: 0,
      }
      current.min = Math.min(current.min, ...values)
      current.max = Math.max(current.max, ...values)
      current.curves += 1
      ranges.set(curve.Id, current)

      const rounded = Math.round(keys[0].value * 100) / 100
      const counter = firstValues.get(curve.Id) ?? new Map<number, number>()
      counter.set(rounded, (counter.get(rounded) ?? 0) + 1)
      firstValues.set(curve.Id, counter)
    }
  }

  const stats = new Map<string, ParameterStats>()
  for (const [id, range] of ranges) {
    stats.set(id, {
      min: range.min,
      max: range.max,
      curves: range.curves,
      base: mostFrequent(firstValues.get(id) ?? new Map()),
    })
  }
  return stats
}
