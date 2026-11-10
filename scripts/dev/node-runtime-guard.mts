import path from 'node:path'
import { fileURLToPath } from 'node:url'

const MIN_NODE: readonly [number, number] = [24, 19]

const isBelowFloor = (version: string) => {
  const [major = 0, minor = 0] = version.split('.').map(Number)
  return major < MIN_NODE[0] || (major === MIN_NODE[0] && minor < MIN_NODE[1])
}

/**
 * Bloqueia `pnpm dev` num Node.js fora do piso de `engines` (`^24.19.0`) — o pnpm só avisa, não
 * barra — e no 24.18.0 do Windows, que derruba o Vite/Rolldown em silêncio. Sobrevive à migração
 * para Tauri porque `tauri dev` ainda sobe o renderer via Vite.
 */
export function getDevRuntimeError(version = process.versions.node, platform = process.platform): string | null {
  if (platform === 'win32' && version === '24.18.0') {
    return `[dev] Node.js ${version} crashes Vite 8/Rolldown on this Windows setup. Upgrade to Node.js 24.19+ LTS.`
  }
  if (isBelowFloor(version)) {
    return `[dev] Node.js ${version} is below the engines floor (^24.19.0). Upgrade to Node.js 24.19+ LTS.`
  }

  return null
}

const entryFile = process.argv[1]

if (entryFile && path.resolve(entryFile) === fileURLToPath(import.meta.url)) {
  const runtimeError = getDevRuntimeError()
  if (runtimeError) {
    console.error(runtimeError)
    process.exitCode = 1
  }
}
