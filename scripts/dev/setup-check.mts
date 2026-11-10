import { spawnSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { getDevRuntimeError } from './node-runtime-guard.mts'

// Mesma lista de `LINUX_DEPS` em .github/workflows/ci.yml, na forma que o pkg-config conhece.
const LINUX_PKG_CONFIG = [
  'webkit2gtk-4.1',
  'ayatana-appindicator3-0.1',
  'librsvg-2.0',
  'openssl',
  'alsa',
  'xscrnsaver',
  'xkbcommon',
  'dbus-1',
  'libpipewire-0.3',
  'gbm',
  'egl',
  'wayland-client',
] as const

const versionOf = (text: string) =>
  text
    .match(/(\d+)\.(\d+)/)
    ?.slice(1, 3)
    .map(Number) ?? [0, 0]
const isOlder = (a: number[], b: number[]) => (a[0] ?? 0) < (b[0] ?? 0) || (a[0] === b[0] && (a[1] ?? 0) < (b[1] ?? 0))

/** O `rust-version` do manifesto é o piso: abaixo dele o build falha no meio, com erro de crate. */
function rustTooOld(): string | null {
  const manifest = readFileSync(new URL('../../apps/desktop/src-tauri/Cargo.toml', import.meta.url), 'utf8')
  const floor = manifest.match(/^rust-version\s*=\s*"([^"]+)"/m)?.[1]
  const out = spawnSync('rustc', ['--version'], { encoding: 'utf8' }).stdout ?? ''
  return floor && isOlder(versionOf(out), versionOf(floor))
    ? `rustc ${versionOf(out).join('.')} abaixo do piso ${floor} — rode \`rustup update stable\``
    : null
}

const works = (cmd: string, args: string[]) => spawnSync(cmd, args, { stdio: 'ignore' }).status === 0

/** Pré-requisitos que faltam para `pnpm dev`, um por linha, já com o que fazer. */
export function findSetupProblems(platform = process.platform, has = works): string[] {
  const problems: string[] = []
  const nodeError = getDevRuntimeError()
  if (nodeError) problems.push(nodeError)
  if (!has('cargo', ['--version'])) problems.push('Rust ausente: instale a toolchain estável em https://rustup.rs')
  else {
    const old = rustTooOld()
    if (old) problems.push(old)
  }
  if (platform === 'linux') {
    if (!has('pkg-config', ['--version'])) problems.push('pkg-config ausente (apt install pkg-config)')
    else {
      const missing = LINUX_PKG_CONFIG.filter((lib) => !has('pkg-config', ['--exists', lib]))
      if (missing.length)
        problems.push(
          `Bibliotecas do Tauri ausentes (pkg-config): ${missing.join(', ')} — veja LINUX_DEPS em .github/workflows/ci.yml`,
        )
    }
  }
  return problems
}

if (process.argv[1]?.endsWith('setup-check.mts')) {
  const problems = findSetupProblems()
  for (const p of problems) console.error(`✗ ${p}`)
  console.log(
    problems.length
      ? `\n${problems.length} pendência(s) antes do primeiro \`pnpm dev\`.`
      : '✓ Ambiente pronto para `pnpm install && pnpm dev`.',
  )
  process.exitCode = problems.length ? 1 : 0
}
