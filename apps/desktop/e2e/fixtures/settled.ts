import { expect, type Page } from '@playwright/test'

export async function waitForSettled(page: Page, selector: string): Promise<void> {
  await expect
    .poll(
      () =>
        page.evaluate((sel) => {
          const element = document.querySelector(sel)
          if (!element) return 0
          let effective = 1
          let node: Element | null = element
          while (node) {
            effective *= Number(getComputedStyle(node).opacity || '1')
            node = node.parentElement
          }
          return Number(effective.toFixed(2))
        }, selector),
      { timeout: 5000 },
    )
    .toBe(1)
}

/**
 * Substitui o `waitForTimeout(<ms>)` que "deixava a animação assentar" antes de um print ou do
 * próximo clique: espera as animações CSS/WAAPI em curso TERMINAREM de fato (`getAnimations`, que
 * inclui as de `framer-motion` em transform/opacity), as fontes carregarem e dois quadros pintados.
 * Determinístico por construção — o tempo gasto é o da animação, não um palpite que passa na
 * máquina rápida e corta a animação na lenta. Para animação de JS puro (sem WAAPI) combine com
 * `waitForSettled(page, selector)` ou com uma asserção do próprio elemento.
 *
 * Só espera animação que TERMINA: as de iteração infinita (`animate-pulse` do BootOverlay,
 * `animate-spin`, `repeat: Infinity` dos indicadores) nunca resolvem `finished`, e as pausadas/ociosas
 * também não — esperar por elas pendurava o spec até o timeout do Playwright. Um teto de 5 s evita
 * que uma animação finita muito longa faça o mesmo.
 */
export async function waitForUiIdle(page: Page): Promise<void> {
  // Uma janela recém-aberta (Configurações, visão de plugin) ainda pode navegar entre o
  // `domcontentloaded` e o primeiro quadro estável: o `evaluate` morre com "Execution context was
  // destroyed". É a página mudando de documento, não um erro do spec — tenta de novo no novo contexto.
  for (let attempt = 1; ; attempt++) {
    try {
      await settleOnce(page)
      return
    } catch (error) {
      if (attempt >= 4 || !/Execution context was destroyed|navigation/i.test(String(error))) throw error
      await page.waitForLoadState('domcontentloaded')
    }
  }
}

async function settleOnce(page: Page): Promise<void> {
  await page.evaluate(async () => {
    await document.fonts.ready
    const finishing = document
      .getAnimations()
      .filter(
        (animation) =>
          animation.playState === 'running' &&
          Number.isFinite(animation.effect?.getComputedTiming().endTime ?? Number.POSITIVE_INFINITY),
      )
    await Promise.race([
      Promise.allSettled(finishing.map((animation) => animation.finished)),
      new Promise<void>((resolve) => setTimeout(resolve, 5000)),
    ])
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())))
  })
}

/**
 * Print de evidência. `fullPage: true` não serve aqui: o conteúdo mora num contêiner interno com
 * scroll (não no documento), então a captura saía com o scroll onde o clique anterior o deixou
 * (cabeçalho cortado) e com uma faixa branca onde o documento passa da janela. Volta todo
 * contêiner ao topo e captura só o que o usuário veria ao abrir a tela.
 */
export async function snapTop(page: Page, path: string): Promise<void> {
  await page.evaluate(() => {
    window.scrollTo(0, 0)
    for (const el of document.querySelectorAll('*')) if (el.scrollTop) el.scrollTop = 0
  })
  await waitForUiIdle(page)
  await page.screenshot({ path })
}
