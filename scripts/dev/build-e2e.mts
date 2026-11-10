import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

/**
 * Binário do E2E (`tauri build --no-bundle --features e2e`), compilado como TESTE, não como release.
 *
 * O perfil `release` do workspace é `lto = true` + `codegen-units = 1`: mudar uma linha do crate do
 * app custava ~15 min de link só para o Playwright abrir a janela. Esse binário nunca é distribuído
 * (a feature `e2e` o proíbe), então aqui o LTO sai e o crate compila em paralelo e incremental —
 * minutos, não quartos de hora. `opt-level = 3` fica (inferência/DSP em Rust puro: otimizado a menos
 * muda o tempo dos specs). Alvo separado (`target/e2e`) para não invalidar o cache do release de
 * verdade; `e2e/fixtures/app-binary.ts` procura o binário ali. `CARGO_TARGET_DIR` do chamador vence.
 */
const root = resolve(fileURLToPath(import.meta.url), '../../..')
const result = spawnSync(
  'pnpm',
  ['exec', 'tauri', 'build', '--no-bundle', '--config', 'src-tauri/tauri.conf.json', '--features', 'e2e'],
  {
    cwd: resolve(root, 'apps/desktop'),
    stdio: 'inherit',
    shell: process.platform === 'win32',
    env: {
      ...process.env,
      // biome-ignore lint/suspicious/noUndeclaredEnvVars: variável do Cargo, não entrada de cache do turbo
      CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? resolve(root, 'target/e2e'),
      CARGO_PROFILE_RELEASE_LTO: 'false',
      CARGO_PROFILE_RELEASE_CODEGEN_UNITS: '16',
      CARGO_PROFILE_RELEASE_INCREMENTAL: 'true',
      CARGO_PROFILE_RELEASE_STRIP: 'false',
    },
  },
)
process.exitCode = result.status ?? 1
