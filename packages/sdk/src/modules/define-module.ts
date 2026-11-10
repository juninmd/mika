// Module manifest declares what it CONSUMES (deps, events, config), not just what it offers; `check:module-manifest` diffs it against actual usage.
// `*.module.ts` must import only from this SDK module — it's read by the gate outside Electron, so any runtime import breaks loading.

/** Homologation profile: library, app adapter, or contract (types/constants, no lifecycle). Absent: `packages/` is library, rest is adapter. */
export type ModuleProfile = 'library' | 'adapter' | 'contract'

export type ModuleLayer = 'perception' | 'cognition' | 'expression' | 'product' | 'platform'

/** Frozen/homologated module. `contract` files are SHA-256 hashed; changing them without reopening homologation fails the gate. */
export interface ModuleFrozen {
  at: string
  hash: string
}

export interface ModuleManifest {
  /** Identificador estável. É por ele que `pnpm homologate <id>` acha o módulo. */
  id: string
  title: string
  layer: ModuleLayer

  /** Pasta raiz do módulo, relativa à raiz do repo. Todo o escaneamento do gate parte daqui. */
  source: string

  /** Arquivos que formam a superfície pública. É o que o congelamento protege. */
  contract: readonly string[]

  /** Tokens do container DI que o módulo resolve (`TOKENS.EventBus` → `'EventBus'`). */
  deps?: readonly string[]

  /** Eventos que o módulo escuta (valores de `Events`, não as chaves). */
  consumes?: readonly string[]

  /** Eventos que o módulo emite. */
  emits?: readonly string[]

  /** Prefixos de config que o módulo pode ler ou gravar, com o ponto: `['voice.']`. */
  config?: readonly string[]

  /** Perfil de homologação, quando a inferência por diretório não vale para este módulo. */
  profile?: ModuleProfile
  /** Filtro `cargo test --lib` do módulo Rust que implementa este comportamento (`modules::features::mood`). */
  rust?: string

  frozen?: ModuleFrozen
}

export function defineModule(manifest: ModuleManifest): ModuleManifest {
  return manifest
}
