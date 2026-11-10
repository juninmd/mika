/**
 * Stub local da Wikipedia — sucessor do `globalThis.fetch` sobrescrito via `app.evaluate` que a
 * suíte Electron usava (`memory-reaction.spec.ts` original). Sem `app.evaluate` (Tauri não tem
 * processo principal alcançável, ver `tauri-app.ts`), o único jeito honesto de a rota
 * `person_catalog::enrichment.rs` (Rust) bater aqui em vez da Wikipedia real é o backend saber, ele
 * mesmo, redirecionar `en.wikipedia.org`/`upload.wikimedia.org` para este servidor — ver
 * `MIKA_E2E_WIKI_BASE_URL` (`src-tauri/src/modules/browser/guarded_fetch/mod.rs::e2e_wiki_override`,
 * dupla trava com `MIKA_E2E=1`, nunca uma chave de config).
 *
 * Serve só `/api/rest_v1/page/summary/<slug>` (o único endpoint que `enrichment.rs` chama —
 * confirmado lendo o arquivo: `WIKI_SUMMARY_API` é a única constante de URL lá). `Wiki404Entity`/
 * `WikiTimeoutEntity` no nome do slug reproduzem os dois caminhos de falha que os testes de
 * fronteira (F1 Boundaries) cobrem.
 */
import { createServer, type Server } from 'node:http'
import type { AddressInfo } from 'node:net'

export interface FakeWiki {
  baseUrl: string
  requests: string[]
  close: () => Promise<void>
}

const EXTRACTS: Record<string, string> = {
  'elden ring': 'Elden Ring is a 2022 action role-playing game developed by FromSoftware.',
  'death note': 'Death Note is a Japanese manga series.',
  'the matrix': 'The Matrix is a 1999 science fiction action film.',
  'breaking bad': 'Breaking Bad is an American crime drama TV series.',
  kojima: 'Hideo Kojima is a Japanese video game designer.',
  'keanu reeves': 'Keanu Reeves is a Canadian actor.',
  'elon musk': 'Elon Musk is the founder of SpaceX.',
  'taylor swift': 'Taylor Swift is a famous singer-songwriter.',
  'lady gaga': 'Lady Gaga is a pop singer and actress.',
}

export async function startFakeWiki(): Promise<FakeWiki> {
  const requests: string[] = []

  const server: Server = createServer((req, res) => {
    const path = (req.url ?? '').split('?')[0] ?? ''
    // The spec's view of what the backend fetched; not itself a backend request.
    if (path === '/__requests') {
      res.writeHead(200, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify(requests))
      return
    }
    requests.push(path)

    const match = path.match(/\/api\/rest_v1\/page\/summary\/(.+)$/)
    if (!match) {
      res.writeHead(404, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ error: `fake-wiki: rota não implementada (${path})` }))
      return
    }

    const slug = decodeURIComponent(match[1] ?? '')
    const name = slug.replace(/_/g, ' ')
    const lower = name.toLowerCase()

    if (lower.includes('wiki404')) {
      res.writeHead(404, { 'Content-Type': 'application/json' })
      res.end(JSON.stringify({ error: 'Not Found' }))
      return
    }

    if (lower.includes('wikitimeout')) {
      // Derruba a conexão na hora (em vez de segurar os 15s do timeout de hop real,
      // `guarded_fetch::HOP_TIMEOUT`) — o resultado do lado Rust é o MESMO branch de erro
      // (`GuardedFetchError::Network`), só não gasta 15s reais por teste.
      req.socket.destroy()
      return
    }

    const matched = Object.entries(EXTRACTS).find(([key]) => lower.includes(key))
    const extract = matched ? matched[1] : `Biography and info about ${name}.`

    res.writeHead(200, { 'Content-Type': 'application/json' })
    res.end(
      JSON.stringify({
        title: name,
        extract,
        thumbnail: { source: `http://127.0.0.1/wiki-${slug}.jpg`, width: 100, height: 100 },
      }),
    )
  })

  await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve))
  const { port } = server.address() as AddressInfo

  return {
    baseUrl: `http://127.0.0.1:${port}`,
    requests,
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections?.()
        server.close(() => resolve())
      }),
  }
}
