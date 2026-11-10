/**
 * check:avatar-parity — pega método de AvatarEngine que existe só num backend (Live2D ou VRM).
 * Não aparece em typecheck (`?.`) nem em teste (ambos exigem WebGL, não sobem fora do navegador).
 * A paridade do vocabulário de expressões mora em `expression-parity.mts`.
 */
import { read, report, type Violation } from './lib.mts'

const AVATAR_CONTRACT = 'apps/desktop/src/renderer/src/components/avatar/avatar-engine.ts'

const ENGINES = [
  { name: 'Live2DEngine', file: 'apps/desktop/src/renderer/src/components/live2d/live2d-engine.ts' },
  { name: 'VrmAvatar', file: 'apps/desktop/src/renderer/src/components/avatar/vrm-adapter.ts' },
]

const quoted = (body: string): Set<string> => new Set([...body.matchAll(/'([^']+)'/g)].map((m) => m[1]))

const add = (v: Violation[], file: string, message: string): number => v.push({ file, line: 1, message })

/** Corpo de um bloco delimitado por `pattern`, ou `''` se o bloco não existir mais. */
const block = (source: string, pattern: RegExp): string => pattern.exec(source)?.[1] ?? ''

function methodParityViolations(contractSource: string): { violations: Violation[]; methods: string[] } {
  const methodsBlock = block(contractSource, /export const AVATAR_ENGINE_METHODS = \[([\s\S]*?)\] as const/)
  if (methodsBlock === '') {
    process.stdout.write(`✗ check:avatar-parity — AVATAR_ENGINE_METHODS não encontrado em ${AVATAR_CONTRACT}\n`)
    process.exit(1)
  }
  const methods = [...quoted(methodsBlock)]
  const interfaceBody = block(contractSource, /export interface AvatarEngine \{([\s\S]*?)\n\}/)
  const declaredNames = new Set([...interfaceBody.matchAll(/^\s{2}([a-zA-Z][\w]*)\(/gm)].map((m) => m[1]))

  const violations: Violation[] = []
  for (const method of methods) {
    if (!declaredNames.has(method)) {
      add(
        violations,
        AVATAR_CONTRACT,
        `AVATAR_ENGINE_METHODS lista "${method}", que a interface AvatarEngine não declara`,
      )
    }
  }
  for (const name of declaredNames) {
    if (!methods.includes(name)) {
      add(
        violations,
        AVATAR_CONTRACT,
        `AvatarEngine declara "${name}", que AVATAR_ENGINE_METHODS não lista — o gate ficaria cego para ele`,
      )
    }
  }
  return { violations, methods }
}

function engineViolations(methods: string[]): Violation[] {
  const violations: Violation[] = []
  for (const engine of ENGINES) {
    const source = read(engine.file)
    if (!new RegExp(`class ${engine.name} implements AvatarEngine`).test(source)) {
      add(violations, engine.file, `${engine.name} não declara "implements AvatarEngine"`)
    }
    if (!/readonly capabilities: AvatarEngineCapabilities/.test(source)) {
      add(
        violations,
        engine.file,
        `${engine.name} não declara suas capabilities — rig de corpo ausente tem de ser dito, não silenciado`,
      )
    }
    for (const method of methods) {
      if (!new RegExp(`^\\s{2}${method}\\(`, 'm').test(source)) {
        add(
          violations,
          engine.file,
          `${engine.name} não implementa "${method}" — o avatar reagiria num backend e não no outro`,
        )
      }
    }
  }
  return violations
}

const contractSource = read(AVATAR_CONTRACT)
const { violations: methodViolations, methods } = methodParityViolations(contractSource)

const violations: Violation[] = [...methodViolations, ...engineViolations(methods)]

report('check:avatar-parity', violations, methods.length * ENGINES.length)
