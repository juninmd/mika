import { existsSync, mkdirSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { appBinaryPath } from './fixtures/app-binary'

// Fails once, up front, instead of once per spec file inside the fixture — same binary check as
// `fixtures/tauri-app.ts`, duplicated here so the whole run aborts immediately with one message.

export default function globalSetup(): void {
  if (!existsSync(appBinaryPath)) {
    throw new Error(`${appBinaryPath} não existe. Rode "pnpm --filter @mika/desktop build:e2e" (tauri build --no-bundle --features e2e) antes de "pnpm test": sem a feature 'e2e' os ganchos de teste não são compilados.`)
  }

  const dir = join(__dirname, 'screenshots')
  rmSync(dir, { force: true, recursive: true })
  mkdirSync(dir, { recursive: true })
}
