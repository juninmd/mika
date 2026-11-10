/**
 * check:settings-nav — pega aba declarada por schema que nenhuma seção do rail alcança (painel
 * existe, compila, sem rota) e item de rail com aba única (o que já inchou o menu para 14 itens).
 * Abas de plugin (runtime, via PluginUiCatalog) ficam fora por construção.
 */
import { isSource, read, report, sourceRoots, type Violation, walk } from './lib.mts'

const NAV_FILE = 'apps/desktop/src/renderer/src/components/settings/MikaNavRail.tsx'

/** `tab: { id: 'character', … }` — a declaração que dá rota a um painel de feature. */
const TAB_DECLARATION = /tab:\s*\{\s*id:\s*'([^']+)'/g

/** Um objeto de `MIKA_NAV_SECTIONS`: id da seção seguido, antes do próximo, da lista `tabIds`. */
const SECTION = /\{\s*id:\s*'([^']+)',[\s\S]*?tabIds:\s*\[([^\]]*)\]/g

const QUOTED = /'([^']+)'/g

const navSource = read(NAV_FILE)

const sectionsStart = navSource.indexOf('MIKA_NAV_SECTIONS: NavSection[]')
const sectionsEnd = navSource.indexOf('BUILT_IN_TAB_IDS')
const sectionsRegion =
  sectionsStart >= 0 && sectionsEnd > sectionsStart ? navSource.slice(sectionsStart, sectionsEnd) : ''

const builtInStart = navSource.indexOf('BUILT_IN_TAB_IDS = new Set([')
const builtInEnd = navSource.indexOf('])', builtInStart)
const builtInRegion = builtInStart >= 0 ? navSource.slice(builtInStart, builtInEnd) : ''

const builtIn = new Set([...builtInRegion.matchAll(QUOTED)].map((m) => m[1] as string))

const sections: Array<{ id: string; tabIds: string[] }> = []
for (const match of sectionsRegion.matchAll(SECTION)) {
  sections.push({
    id: match[1] as string,
    tabIds: [...(match[2] as string).matchAll(QUOTED)].map((m) => m[1] as string),
  })
}

const declared = new Map<string, string>()
for (const file of sourceRoots().flatMap((root) => walk(root, isSource))) {
  if (!file.endsWith('.schema.ts')) continue
  for (const match of read(file).matchAll(TAB_DECLARATION)) {
    const tabId = match[1] as string
    if (!declared.has(tabId)) declared.set(tabId, file)
  }
}

// Stage C (2026-09-22): a maioria das declarações de aba migrou de `.schema.ts` para Rust
// (`declare_namespace`/`json!({ "tabId": "…", … })`) junto com `src/main` e o não-renderer de
// `src/features/*`. Mesmo padrão literal — `"tabId": "valor"` — tanto na declaração de produção
// quanto na asserção de teste que a lê de volta, então uma varredura simples basta.
// `declare_namespace(schema, "ns", "tab", …)` passes the tab positionally (mood, since its `.schema.ts` left).
const RUST_TAB = /"tabId":\s*"([^"]+)"|declare_namespace\(\s*\w+,\s*"[^"]+",\s*"([^"]+)"/g
for (const file of walk('apps/desktop/src-tauri/src', (rel) => rel.endsWith('.rs') && !/_tests?\.rs$/.test(rel))) {
  const full = read(file)
  if (full.split('\n').some((line) => line.trim() === '#![cfg(test)]')) continue
  const cut = full.indexOf('#[cfg(test)]')
  const source = cut === -1 ? full : full.slice(0, cut)

  for (const match of source.matchAll(RUST_TAB)) {
    const tabId = (match[1] ?? match[2]) as string
    if (!declared.has(tabId)) declared.set(tabId, file)
  }
}

const violations: Violation[] = []
const owner = new Map<string, string>()

for (const section of sections) {
  if (section.tabIds.length < 2) {
    violations.push({
      file: NAV_FILE,
      line: 1,
      message: `a seção "${section.id}" tem ${section.tabIds.length} aba(s) — item de rail com aba única é o que inchou o menu; agrupe ou funda na seção vizinha`,
    })
  }
  for (const tabId of section.tabIds) {
    const previous = owner.get(tabId)
    if (previous !== undefined) {
      violations.push({
        file: NAV_FILE,
        line: 1,
        message: `a aba "${tabId}" aparece em duas seções ("${previous}" e "${section.id}")`,
      })
      continue
    }
    owner.set(tabId, section.id)
  }
}

for (const [tabId, file] of declared) {
  if (owner.has(tabId)) continue
  violations.push({
    file,
    line: 1,
    message: `a aba "${tabId}" é declarada no schema e nenhuma seção do rail a alcança — painel sem rota de navegação`,
  })
}

for (const tabId of builtIn) {
  if (owner.has(tabId)) continue
  violations.push({
    file: NAV_FILE,
    line: 1,
    message: `a aba embutida "${tabId}" não aparece em nenhuma seção do rail`,
  })
}

for (const [tabId, sectionId] of owner) {
  if (declared.has(tabId) || builtIn.has(tabId)) continue
  violations.push({
    file: NAV_FILE,
    line: 1,
    message: `a seção "${sectionId}" aponta para a aba "${tabId}", que não é declarada por schema nem embutida`,
  })
}

// Guarda do próprio extrator: um parse que deixou de casar passa vazio e vira cobertura falsa.
if (declared.size < 14) {
  violations.push({
    file: 'scripts/checks/settings-nav.mts',
    line: 1,
    message: `só ${declared.size} abas reconhecidas nos schemas (eram 14); o extrator deixou de casar — corrija TAB_DECLARATION antes de confiar neste gate`,
  })
}
if (sections.length === 0) {
  violations.push({
    file: 'scripts/checks/settings-nav.mts',
    line: 1,
    message: 'nenhuma seção reconhecida em MIKA_NAV_SECTIONS; o extrator deixou de casar',
  })
}

report('check:settings-nav', violations, declared.size + sections.length)
