import { t } from '@mika/sdk/i18n'
import { parse as parseToml, stringify as stringifyToml } from 'smol-toml'
import { downloadBlobAs } from '../../lib/download-blob'
import { isSensitivePath } from '../../lib/sensitive-paths'

export interface BackupField {
  path: string
  value: unknown
}

const BACKUP_VERSION = 1

// `__configMigrations` é estado interno de migração, não preferência do usuário — nunca exportar/importar.
const EXCLUDED_NAMESPACES = new Set(['__configMigrations'])

/** Achata o config em pares `namespace.campo` — um nível, o mesmo grão que cada `configPath` da UI usa. */
export function flattenConfigFields(config: Record<string, unknown>): BackupField[] {
  const fields: BackupField[] = []
  for (const [namespace, group] of Object.entries(config)) {
    if (EXCLUDED_NAMESPACES.has(namespace)) continue
    if (group === null || typeof group !== 'object' || Array.isArray(group)) continue
    for (const [key, value] of Object.entries(group as Record<string, unknown>)) {
      fields.push({ path: `${namespace}.${key}`, value })
    }
  }
  return fields
}

export interface BackupPayload {
  version: number
  exportedAt: string
  includesSecrets: boolean
  fields: Record<string, unknown>
}

export function buildBackupPayload(config: Record<string, unknown>, includeSecrets: boolean): BackupPayload {
  const fields = flattenConfigFields(config).filter((field) => includeSecrets || !isSensitivePath(field.path))
  return {
    version: BACKUP_VERSION,
    exportedAt: new Date().toISOString(),
    includesSecrets: includeSecrets,
    fields: Object.fromEntries(fields.map((field) => [field.path, field.value])),
  }
}

/** Só valida a forma do arquivo — quem decide se cada campo é aplicável é quem já tem o schema: o import store. */
export function parseBackupPayload(raw: unknown): Record<string, unknown> {
  if (!raw || typeof raw !== 'object') throw new Error(t('settings.backup.error.notObject'))
  const fields = (raw as Record<string, unknown>).fields
  if (!fields || typeof fields !== 'object' || Array.isArray(fields)) {
    throw new Error(t('settings.backup.error.badFields'))
  }
  return fields as Record<string, unknown>
}

/**
 * TOML é o formato padrão: o arquivo é editado à mão (trocar uma chave de API, apagar uma seção) e
 * cada namespace vira uma tabela `[fields.<ns>]` — o mesmo grão de `namespace.campo` da UI. JSON
 * continua aceito na leitura para os backups exportados antes desta troca. INI foi descartado: não
 * tem tipo (tudo é string) nem aninhamento, e há campos que são listas ou objetos.
 */
export type BackupFormat = 'toml' | 'json'

/** Extensão manda; sem ela, um `{` inicial só pode ser JSON — TOML válido nunca começa assim. */
export function backupFormatFor(filename: string, text: string): BackupFormat {
  const ext = filename.toLowerCase().split('.').pop()
  if (ext === 'json' || ext === 'toml') return ext
  return text.trimStart().startsWith('{') ? 'json' : 'toml'
}

/** TOML não tem `null`: um campo nulo sai do arquivo, e na volta o valor atual simplesmente fica. */
function withoutNulls(value: unknown): unknown {
  if (Array.isArray(value)) return value.filter((item) => item != null).map(withoutNulls)
  if (value === null || typeof value !== 'object') return value
  return Object.fromEntries(
    Object.entries(value)
      .filter(([, inner]) => inner != null)
      .map(([key, inner]) => [key, withoutNulls(inner)]),
  )
}

/** `{ 'voice.volume': 80 }` → `{ voice: { volume: 80 } }`, inverso de `flattenConfigFields`. */
function groupByNamespace(fields: Record<string, unknown>): Record<string, Record<string, unknown>> {
  const grouped: Record<string, Record<string, unknown>> = {}
  for (const [path, value] of Object.entries(fields)) {
    const dot = path.indexOf('.')
    const namespace = path.slice(0, dot)
    grouped[namespace] ??= {}
    grouped[namespace][path.slice(dot + 1)] = value
  }
  return grouped
}

export function serializeBackup(payload: BackupPayload, format: BackupFormat): string {
  if (format === 'json') return JSON.stringify(payload, null, 2)
  return stringifyToml(
    withoutNulls({ ...payload, fields: groupByNamespace(payload.fields) }) as Record<string, unknown>,
  )
}

/** Lê o texto do arquivo escolhido e devolve os pares `namespace.campo` a aplicar. */
export function parseBackupText(text: string, filename: string): Record<string, unknown> {
  if (backupFormatFor(filename, text) === 'json') return parseBackupPayload(JSON.parse(text))
  let raw: Record<string, unknown>
  try {
    raw = parseToml(text)
  } catch {
    throw new Error(t('settings.backup.error.badToml'))
  }
  const grouped = parseBackupPayload(raw)
  return Object.fromEntries(flattenConfigFields(grouped).map((field) => [field.path, field.value]))
}

export function downloadBackup(payload: BackupPayload, format: BackupFormat): void {
  const type = format === 'json' ? 'application/json' : 'application/toml'
  const blob = new Blob([serializeBackup(payload, format)], { type })
  downloadBlobAs(blob, `mika-settings-${new Date().toISOString().slice(0, 10)}.${format}`)
}
