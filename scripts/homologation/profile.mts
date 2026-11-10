/**
 * Perfis de homologação — §10.3 do plano modular. Os critérios OBRIGATÓRIOS mudam por perfil; um
 * critério informativo nunca reprova. Headless vale para todos: contrato que só carrega dentro do
 * Electron não se testa sem subir o app (o último caso era um `getElectronApi()` no topo do módulo).
 */
export type Profile = 'library' | 'adapter' | 'contract'

export type CriterionId = 'fronteira' | 'vivo' | 'headless' | 'import-publico' | 'montagem' | 'config-ui' | 'contrato'

/**
 * Critérios obrigatórios por perfil. Biblioteca que não carrega fora do Electron, não expõe
 * entrypoint importável ou não prova montar e desmontar uma instância não é biblioteca — é código do
 * app morando em `packages/`. `config-ui` só é computado (ver homologate.mts) quando o módulo declara
 * schema de configuração — por isso entra como obrigatório nos dois perfis sem quebrar quem não tem UI.
 */
export const REQUIRED: Record<Profile, readonly CriterionId[]> = {
  library: ['fronteira', 'headless', 'import-publico', 'montagem', 'config-ui', 'contrato'],
  adapter: ['fronteira', 'headless', 'config-ui', 'contrato'],
  // Types, IPC contracts and constants only: nothing to mount, so `montagem` would be vacuous.
  contract: ['fronteira', 'headless', 'import-publico', 'config-ui', 'contrato'],
}

/** Sem declaração explícita, quem mora em `packages/` é biblioteca; o resto é adaptador do app. */
export function profileOf(source: string, declared?: Profile): Profile {
  return declared ?? (source.startsWith('packages/') ? 'library' : 'adapter')
}

export type Status = 'ok' | 'falhou' | 'ausente'

export interface CriterionResult {
  id: CriterionId
  status: Status
  detail: string
  /** Saída bruta (log/stdout) quando falhou — evidência, não decoração. */
  evidence?: string
}

/** Aprovado exige TODO critério obrigatório executado e verde: ausência não vira aprovação. */
export function verdict(profile: Profile, results: readonly CriterionResult[]): 'aprovado' | 'reprovado' {
  const required = REQUIRED[profile]
  return results.every((r) => !required.includes(r.id) || r.status === 'ok') ? 'aprovado' : 'reprovado'
}
