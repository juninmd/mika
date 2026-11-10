/**
 * check:config-schema-parity — "schema → lista", a metade que faltava de
 * `specs/09-settings-and-ui.md` ("Cobertura de campos nos dois sentidos"). O irmão "lista → JSX" já
 * existe (`settings-render-coverage.test.ts`); sem este, dava para declarar um painel com metade
 * dos campos e a auditoria ficava verde — foi assim que doze campos de schema ficaram sem UI
 * alguma antes do gate irmão nascer.
 *
 * O nome já era citado nos comentários de `config/schemas/*.rs` (`gamer.rs`, `llm.rs`, `mod.rs`)
 * como se o script existisse — não existia. Doc/comentário afirmando gate que não roda é a mesma
 * classe de defeito que `check:doc-paths` guarda para caminho de arquivo.
 *
 * Extração por regex, na mesma forma que os comentários do Rust já prescrevem: `name: "X",` colado
 * (só espaço em branco) em `fields: vec![`, e o `]` de fechamento isolado em linha própria — um
 * `vec![...]` mal formatado vaza campos para o namespace vizinho (já aconteceu: `memory.*` vazou
 * para dentro de `scheduler`).
 */
import { configSchemaLeafPaths, read, report, type Violation, walk } from './lib.mts'

const HANDLED_FILES = [
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields.ts',
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields-profile.ts',
  'apps/desktop/src/renderer/src/components/settings/feature-component-fields-voice.ts',
]

/** Toggle mestre de uma aba — `enabledConfigPath` na declaração Rust (`brain_tab.rs`,
 * `browser/settings.rs`, `mcp/feature.rs`, `person_catalog/feature.rs`, ...). Cobertura real, mas
 * fora do dicionário acima: `SettingsTabHero.tsx` renderiza o controle genericamente (um
 * `SettingsFieldCard` boolean por aba com `enabledConfigPath`), não por entrada em
 * `FEATURE_COMPONENT_HANDLED_FIELDS` — confirmado lendo o componente, não assumido. Sem isto,
 * TODO toggle mestre de aba nova nasceria "sem UI" apesar de a Hero já desenhar o controle.
 */
function tabHeroHandledPaths(): Set<string> {
  const ENABLED_CONFIG_PATH = /"enabledConfigPath":\s*"([^"]+)"/g
  const paths = new Set<string>()
  for (const file of walk('apps/desktop/src-tauri/src/modules', (rel) => rel.endsWith('.rs'))) {
    for (const match of read(file).matchAll(ENABLED_CONFIG_PATH)) {
      paths.add(match[1] as string)
    }
  }
  return paths
}

/** Campo de schema legítimo sem UI de Settings (dicionário/cache interno, ou controlado por um
 * diálogo próprio fora do sistema de abas de Settings). Motivo por campo, nunca só a chave. */
const EXEMPT: ReadonlyMap<string, string> = new Map([
  ['app.locale', 'escolhido no wizard (LanguageStep.tsx), não reeditável por painel de Settings'],
  [
    'app.performanceProfile',
    'renderizado por PerformanceProfileCard.tsx (aba Resumo), fora do dicionário de HANDLED_FIELDS',
  ],
  [
    'app.inferenceAcceleration',
    'renderizado por PerformanceProfileCard.tsx (aba Resumo), fora do dicionário de HANDLED_FIELDS',
  ],
  [
    'app.fullscreen',
    'renderizado por WindowModeCard.tsx (aba Resumo) pelo canal window-toggle-fullscreen, que grava a chave; fora do dicionário de HANDLED_FIELDS',
  ],
  [
    'app.autoUpdate',
    'renderizado por UpdatePanel.tsx (aba Atualizações, AutoUpdateSection), fora do dicionário de HANDLED_FIELDS',
  ],
  ['setup.wizardCompleted', 'flag de máquina de estado do wizard, não campo que o usuário edita'],
  ['setup.wizardCurrentStep', 'flag de máquina de estado do wizard, não campo que o usuário edita'],
  [
    'legal.acceptedVersion',
    'gravado pelo aceite de EULA/privacidade (LegalConsent.tsx: wizard e LegalGate), não editável',
  ],
  ['legal.acceptedAt', 'carimbo do mesmo aceite (LegalConsent.tsx), nunca editado por painel de Settings'],
  [
    'legal.ageConfirmed',
    'confirmação "18 anos ou mais" gravada pelo mesmo aceite (LegalConsent.tsx), nunca editada por painel de Settings',
  ],
  ['telemetry.enabled', 'controlado pelo diálogo TelemetryOptIn.tsx, fora do sistema de abas de Settings'],
  ['telemetry.optInShown', 'bookkeeping interno de TelemetryOptIn.tsx (diálogo já mostrado?)'],
  ['telemetry.sessionId', 'id interno gerado, nunca exposto para edição'],
  // UI real existe (AudioDevicesPanel.tsx, DeviceSelect); só não está listada em
  // FEATURE_COMPONENT_HANDLED_FIELDS — lacuna de contabilidade, não de produto.
  ['audio.inputDeviceId', 'renderizado por AudioDevicesPanel.tsx (DeviceSelect), fora do dicionário de HANDLED_FIELDS'],
  [
    'audio.outputDeviceId',
    'renderizado por AudioDevicesPanel.tsx (DeviceSelect), fora do dicionário de HANDLED_FIELDS',
  ],
  // Varredura de controles de UI de 2026-09-27: cache interno sem produtor nem consumidor em
  // apps/desktop/src-tauri/src nem apps/desktop/src — removido de GamerCompanionPanel (era um
  // <input> editável sem efeito nenhum). Campo de schema mantido: `ConfigStore` guarda chave
  // desconhecida sem erro (AGENTS.md), e um consumidor real futuro reusa o valor já persistido.
  ['gamer-companion.topPlayedCache', 'cache interno sem UI própria — ver modules/gamer/feature.rs'],
])

const schemaFields = configSchemaLeafPaths() // path -> arquivo de origem

const handled = new Set<string>()
const PATH_LITERAL = /'([a-zA-Z][\w-]*(?:\.[\w-]+)+)'/g
for (const file of HANDLED_FILES) {
  for (const match of read(file).matchAll(PATH_LITERAL)) {
    handled.add(match[1] as string)
  }
}
for (const path of tabHeroHandledPaths()) {
  handled.add(path)
}

const violations: Violation[] = []

for (const [path, file] of schemaFields) {
  if (handled.has(path)) continue
  if (EXEMPT.has(path)) continue
  violations.push({
    file,
    line: 1,
    message: `campo de schema "${path}" não aparece em nenhum FEATURE_COMPONENT_HANDLED_FIELDS — painel sem cobertura, ou isente em scripts/checks/config-schema-parity.mts com motivo`,
  })
}

for (const [path] of EXEMPT) {
  if (!schemaFields.has(path)) {
    violations.push({
      file: 'scripts/checks/config-schema-parity.mts',
      line: 1,
      message: `isenção "${path}" não corresponde a nenhum campo de schema — isenção obsoleta`,
    })
  }
}

// Piso de alcance: um regex que pare de casar não deve sair verde silenciosamente.
if (schemaFields.size < 200) {
  violations.push({
    file: 'scripts/checks/config-schema-parity.mts',
    line: 1,
    message: `só ${schemaFields.size} campo(s) de schema reconhecido(s) — piso de alcance quebrado, confira NAMESPACE_BLOCK/FIELD_KEY antes de confiar no resultado`,
  })
}

report('check:config-schema-parity', violations, schemaFields.size)
