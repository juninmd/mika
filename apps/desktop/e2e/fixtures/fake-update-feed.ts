/**
 * `latest.json` local para o updater do Tauri — o spec de aviso de versão nova aponta o backend
 * para cá via `MIKA_E2E_UPDATE_ENDPOINT` (`src-tauri/src/updater.rs`, dupla trava com `MIKA_E2E=1`).
 * Só o CHECK passa por aqui: o spec nunca clica em "Atualizar agora", então url/assinatura não
 * precisam baixar nada de verdade, só existir para a plataforma corrente.
 */
import { createServer } from 'node:http'
import type { AddressInfo } from 'node:net'

export interface FakeUpdateFeed {
  endpoint: string
  hits: () => number
  close: () => Promise<void>
}

const PLATFORMS = ['linux-x86_64', 'windows-x86_64', 'darwin-x86_64', 'darwin-aarch64']

export async function startFakeUpdateFeed(version: string, notes: string): Promise<FakeUpdateFeed> {
  let hits = 0
  const platforms = Object.fromEntries(
    PLATFORMS.map((p) => [p, { signature: 'e2e-unsigned', url: `http://127.0.0.1:9/mika-${version}.bin` }]),
  )
  const body = JSON.stringify({ version, notes, pub_date: '2026-11-17T00:00:00Z', platforms })
  const server = createServer((_req, res) => {
    hits++
    res.writeHead(200, { 'content-type': 'application/json' })
    res.end(body)
  })
  await new Promise<void>((resolveListen) => server.listen(0, '127.0.0.1', resolveListen))
  const { port } = server.address() as AddressInfo
  return {
    endpoint: `http://127.0.0.1:${port}/latest.json`,
    hits: () => hits,
    close: () => new Promise<void>((resolveClose) => server.close(() => resolveClose())),
  }
}
