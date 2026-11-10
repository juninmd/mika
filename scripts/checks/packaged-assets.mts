/**
 * check:packaged-assets — pega asset de terceiro (modelos Live2D) sem exclusão do bundle, e asset
 * sem entrada no inventário de licenças. Exclusão é uma linha só, e uma linha só é o tipo de coisa
 * que volta sem ninguém perceber.
 *
 * Stage C (2026-09-22): o electron-builder (`apps/desktop/package.json#build.files`) saiu. O
 * Tauri empacota `frontendDist` (`out/renderer`) por inteiro — não tem um `files`/glob de exclusão
 * equivalente no `tauri.conf.json`. **Isto ainda é um buraco real**: `public/live2d` vai para
 * `out/renderer/live2d` no build do Vite e, sem um passo que apague esse diretório do bundle antes
 * do `tauri build` empacotar, o instalador volta a redistribuir modelos de terceiro sem licença —
 * a mesma classe de defeito que este gate existia para pegar. Ver relatório da migração.
 */
import { readdirSync } from 'node:fs'
import { resolve } from 'node:path'
import { exists, REPO_ROOT, read, report, type Violation } from './lib.mts'

const INVENTORY = 'LICENSE-ASSETS.md'
// Onde a exclusão TEM de existir agora: um passo do pipeline de build do Tauri que remova o
// diretório do bundle após o Vite copiar `public/live2d` para `out/renderer/live2d`.
const BUILD_PIPELINE = 'apps/desktop/vite.tauri.config.ts'

/** Diretórios de asset de terceiro que MUST NOT ser distribuídos. */
const THIRD_PARTY_ASSET_DIRS = [{ publicDir: 'apps/desktop/src/renderer/public/live2d', outDir: 'out/renderer/live2d' }]

const inventory = read(INVENTORY)
const violations: Violation[] = []

// O frontendDist serve o avatar, mas o inspetor Rust lê Resource/renderer/vrm no disco.
// Sem esta cópia, o modelo oficial aparece e sua inspeção falha no instalador.
const TAURI_CONFIG = 'apps/desktop/src-tauri/tauri.conf.json'
const resources = JSON.parse(read(TAURI_CONFIG)).bundle?.resources
if (resources?.['../out/renderer/vrm/'] !== 'renderer/vrm/') {
  violations.push({
    file: TAURI_CONFIG,
    line: 1,
    message: 'bundle.resources precisa preservar vrm/ em renderer/vrm/ para o inspetor do avatar oficial',
  })
}

for (const asset of THIRD_PARTY_ASSET_DIRS) {
  const pipeline = exists(BUILD_PIPELINE) ? read(BUILD_PIPELINE) : ''
  if (!pipeline.includes(asset.outDir)) {
    violations.push({
      file: BUILD_PIPELINE,
      line: 1,
      message: `nenhum passo do build remove "${asset.outDir}" antes do bundle Tauri — o instalador redistribuiria ${asset.publicDir}`,
    })
    continue
  }

  // Cada modelo presente na árvore tem de estar nomeado no inventário de licenças.
  let entries: string[]
  try {
    entries = readdirSync(resolve(REPO_ROOT, asset.publicDir), { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
  } catch {
    continue
  }

  for (const model of entries) {
    if (!inventory.includes(`${model}/`)) {
      violations.push({
        file: INVENTORY,
        line: 1,
        message: `${asset.publicDir}/${model} existe e não está no inventário de licenças`,
      })
    }
  }
}

report('check:packaged-assets', violations, THIRD_PARTY_ASSET_DIRS.length)
