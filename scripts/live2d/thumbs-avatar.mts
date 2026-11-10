import { createHash } from 'node:crypto'
import { readFileSync } from 'node:fs'
import type { Page } from '@playwright/test'

const CONTAINER = '.live2d-container'
const AVATAR_TIMEOUT_MS = 90_000

const FALLBACK_WARN = 'Tentando fallback'

export { AVATAR_TIMEOUT_MS, CONTAINER }

const watched = new WeakSet<Page>()

function watchFallback(page: Page, sink: string[]): void {
  if (watched.has(page)) return
  watched.add(page)
  page.on('console', (message) => {
    const text = message.text()
    if (text.includes(FALLBACK_WARN)) sink.push(text)
  })
}

export async function findAvatarWindow(pages: () => Page[], sink: string[]): Promise<Page> {
  const deadline = Date.now() + AVATAR_TIMEOUT_MS
  while (Date.now() < deadline) {
    for (const page of pages()) {
      watchFallback(page, sink)
      const found = await page
        .locator(CONTAINER)
        .count()
        .catch(() => 0)
      if (found > 0) return page
    }
    await new Promise((resolve) => setTimeout(resolve, 250))
  }
  throw new Error(`nenhuma janela montou ${CONTAINER} — só a principal tem avatar`)
}

export async function waitForAvatar(page: Page): Promise<void> {
  await page.waitForFunction(
    () => {
      const bridge = (window as unknown as { __mikaLive2D?: { listActions(): string[] } }).__mikaLive2D
      return (bridge?.listActions().length ?? 0) > 0
    },
    undefined,
    { timeout: AVATAR_TIMEOUT_MS },
  )
}

export async function stretchStageToWindow(page: Page): Promise<void> {
  await page.evaluate(() => window.dispatchEvent(new Event('resize')))
  await page.waitForFunction(
    () => {
      const canvas = document.querySelector(`.live2d-container canvas`)
      if (!(canvas instanceof HTMLCanvasElement)) return false
      return canvas.clientWidth === window.innerWidth && canvas.clientHeight === window.innerHeight
    },
    undefined,
    { timeout: 15_000 },
  )

  await page.waitForTimeout(600)
}

export async function assertSkinIdentity(
  page: Page,
  model: { path: string; label: string; catalogKey: string },
  fallbacks: string[],
  actionCatalog: Record<string, Record<string, unknown>>,
): Promise<void> {
  if (fallbacks.length > 0) {
    throw new Error(`o loader caiu na skin padrão ao pedir '${model.path}': ${fallbacks[0]}`)
  }

  const expected = Object.keys(actionCatalog[model.catalogKey] ?? {})
  if (expected.length === 0) {
    throw new Error(`o catálogo gerado não tem '${model.catalogKey}'. Rode: pnpm live2d:gen`)
  }
  const loaded = await page.evaluate(() => {
    const bridge = (window as unknown as { __mikaLive2D?: { listActions(): string[] } }).__mikaLive2D
    return bridge?.listActions() ?? []
  })
  const same = loaded.length === expected.length && expected.every((action) => loaded.includes(action))
  if (!same) {
    throw new Error(
      `a skin carregada não é ${model.label}: o loader caiu no modelo padrão (só um warn no console). ` +
        `Ações esperadas [${expected.join(', ')}], carregadas [${loaded.join(', ')}]`,
    )
  }
}

export function assertPreviewsAreDistinct(files: { label: string; file: string }[]): void {
  const byDigest = new Map<string, string[]>()
  for (const { label, file } of files) {
    const digest = createHash('sha256').update(readFileSync(file)).digest('hex')
    byDigest.set(digest, [...(byDigest.get(digest) ?? []), label])
  }
  const clones = [...byDigest.values()].filter((labels) => labels.length > 1)
  if (clones.length > 0) {
    const groups = clones.map((labels) => labels.join(' = ')).join(' · ')
    throw new Error(`miniaturas idênticas, ou seja, a mesma skin fotografada mais de uma vez: ${groups}`)
  }
}
