import { resolve } from 'node:path'

/**
 * Caminho do binário release que a suíte E2E, a homologação e os scripts de captura
 * (`scripts/lib/tauri-capture-app.mts`) dirigem — o ÚNICO lugar que o resolve.
 *
 * `pnpm build:e2e` compila em `<raiz>/target/e2e/release/` (sem LTO, ver `scripts/dev/build-e2e.mts`: o
 * release de verdade levava ~15 min por mudança) e não mais em
 * `<raiz>/target/release/` — nem, desde o workspace, em
 * `apps/desktop/src-tauri/target/` (quatro cópias do caminho velho apontavam para um binário que
 * nunca mais é gerado). `CARGO_TARGET_DIR`, quando definido, vence — é o que o próprio Cargo faz.
 * Gere com `pnpm --filter @mika/desktop build:e2e` (= `tauri build --no-bundle --features e2e`): a feature
 * `e2e` compila os ganchos de teste (`mika:e2e-hook`, override da Wikipedia) que o binário distribuído NÃO
 * tem — sem ela a suíte falha nos specs que dependem deles. NUNCA `cargo build --release` cru:
 * `tauri::is_dev()` é `!cfg!(feature = "custom-protocol")` e quem liga essa feature é o CLI; um binário
 * de `cargo build` cru se acha em desenvolvimento e carrega o `devUrl` em vez do bundle embarcado.
 * `__dirname`: Playwright compila `e2e/**` em CJS e o `tsx` dos scripts também trata este `.ts` assim.
 */
const REPO_ROOT = resolve(__dirname, '../../../..')
const TARGET_DIR = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : resolve(REPO_ROOT, 'target/e2e')

export const appBinaryPath = resolve(TARGET_DIR, 'release', process.platform === 'win32' ? 'mika-desktop.exe' : 'mika-desktop')
