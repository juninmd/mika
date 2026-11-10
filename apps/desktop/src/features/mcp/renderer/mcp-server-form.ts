import type { McpServerSpecInput, McpServerSummary } from '@mika/sdk/contracts'
import type { McpTemplate } from './mcp-templates'

/**
 * Formulário de servidor MCP ↔ `McpServerSpecInput`. A lista nunca devolve valores de env, só as
 * chaves: uma linha `saved` deixada em branco vira `null` ("mantenha o segredo salvo").
 */
export type EnvRow = { id: string; key: string; value: string; saved: boolean }

export type McpServerForm = {
  name: string
  command: string
  args: string[]
  env: EnvRow[]
  cwd: string
  enabled: boolean
  /** Args obrigatórios (vindo de template), ex.: a pasta do filesystem. */
  minArgs?: number
}

export type McpFormErrorCode =
  | 'required'
  | 'nameChars'
  | 'nameLength'
  | 'nameTaken'
  | 'cwdAbsolute'
  | 'argsMissing'
  | 'envKey'
  | 'envDuplicate'
  | 'envValue'

/** Campo (`name`, `command`, `cwd`, `args`, `env.<rowId>`) → código de erro. */
export type McpFormErrors = Record<string, McpFormErrorCode>

const NAME_RE = /^[A-Za-z0-9_.-]+$/
const ENV_KEY_RE = /^[A-Za-z_][A-Za-z0-9_]{0,127}$/
const ABSOLUTE_RE = /^(\/|[A-Za-z]:[\\/]|\\\\)/

export const emptyForm = (): McpServerForm => ({ name: '', command: '', args: [], env: [], cwd: '', enabled: true })

export function formFromServer(server: McpServerSummary): McpServerForm {
  return {
    name: server.name,
    command: server.command,
    args: [...server.args],
    env: server.envKeys.map((key) => ({ id: key, key, value: '', saved: true })),
    cwd: server.cwd ?? '',
    enabled: server.enabled,
  }
}

/** Mesma equivalência de `sanitize` no Rust: `my-srv` e `my_srv` colidem. */
const sanitize = (name: string): string => name.replace(/[^A-Za-z0-9_]/g, '_')

function freeName(base: string, taken: readonly string[]): string {
  const used = new Set(taken.map(sanitize))
  if (!used.has(sanitize(base))) return base
  let n = 2
  while (used.has(sanitize(`${base}-${n}`))) n += 1
  return `${base}-${n}`
}

export function formFromTemplate(template: McpTemplate, taken: readonly string[]): McpServerForm {
  return {
    ...emptyForm(),
    name: freeName(template.serverName, taken),
    command: template.command,
    args: [...template.args],
    env: template.env.map((key) => ({ id: key, key, value: '', saved: false })),
    minArgs: template.minArgs,
  }
}

function envFromRows(rows: readonly EnvRow[]): Record<string, string | null> {
  const env: Record<string, string | null> = {}
  for (const row of rows) {
    const key = row.key.trim()
    if (!key) continue
    env[key] = row.saved && row.value === '' ? null : row.value
  }
  return env
}

export function specFromForm(form: McpServerForm): McpServerSpecInput {
  const cwd = form.cwd.trim()
  return {
    command: form.command.trim(),
    args: form.args.filter((arg) => arg !== ''),
    env: envFromRows(form.env),
    cwd: cwd || null,
    enabled: form.enabled,
  }
}

/** Liga/desliga sem abrir o formulário: todo segredo salvo segue como `null`. */
export function toggledSpec(server: McpServerSummary, enabled: boolean): McpServerSpecInput {
  return { ...specFromForm(formFromServer(server)), enabled }
}

/** Divide uma linha de comando respeitando aspas simples/duplas; `\\` fica literal (caminho Windows). */
export function splitCommandLine(line: string): string[] {
  // Token = sequência de trechos entre aspas e/ou sem espaço; aspas vazias (`""`) contam como arg vazio.
  const tokens = line.match(/(?:"[^"]*"?|'[^']*'?|[^\s"']+)+/g) ?? []
  return tokens.map((token) => token.replace(/"([^"]*)"?|'([^']*)'?/g, (_, dq, sq) => dq ?? sq ?? ''))
}

/** Colou `npx -y pacote` no campo de comando: vira comando + args (antes dos args já existentes). */
export function pasteCommandLine(form: McpServerForm, line: string): McpServerForm {
  const [command = '', ...args] = splitCommandLine(line)
  return { ...form, command, args: [...args, ...form.args] }
}

const quoteArg = (arg: string): string => (/[\s"']/.test(arg) || arg === '' ? JSON.stringify(arg) : arg)

export function commandPreview(command: string, args: readonly string[]): string {
  return [command, ...args].map(quoteArg).join(' ')
}

function nameError(name: string, taken: readonly string[], originalName?: string): McpFormErrorCode | undefined {
  const trimmed = name.trim()
  if (!trimmed) return 'required'
  // Unchanged name of an entry already in `mcp.json`: the backend accepts it as-is (`servers.rs`).
  if (originalName !== undefined && name === originalName) return undefined
  if (trimmed.length > 64) return 'nameLength'
  if (!NAME_RE.test(trimmed)) return 'nameChars'
  const wanted = sanitize(trimmed)
  if (taken.some((other) => other !== originalName && sanitize(other) === wanted)) return 'nameTaken'
  return undefined
}

function envErrors(rows: readonly EnvRow[]): McpFormErrors {
  const errors: McpFormErrors = {}
  const seen = new Set<string>()
  for (const row of rows) {
    const key = row.key.trim()
    if (!key && !row.value) continue
    if (!ENV_KEY_RE.test(key)) errors[`env.${row.id}`] = 'envKey'
    else if (seen.has(key)) errors[`env.${row.id}`] = 'envDuplicate'
    else if (!row.saved && row.value === '') errors[`env.${row.id}`] = 'envValue'
    seen.add(key)
  }
  return errors
}

export function validateForm(form: McpServerForm, taken: readonly string[], originalName?: string): McpFormErrors {
  const errors: McpFormErrors = envErrors(form.env)
  const name = nameError(form.name, taken, originalName)
  if (name) errors.name = name
  if (!form.command.trim()) errors.command = 'required'
  const cwd = form.cwd.trim()
  if (cwd && !ABSOLUTE_RE.test(cwd)) errors.cwd = 'cwdAbsolute'
  if (form.minArgs && form.args.filter(Boolean).length < form.minArgs) errors.args = 'argsMissing'
  return errors
}

export type McpErrorHint = 'notFoundNode' | 'notFoundUv' | 'notFound' | 'timeout' | 'env' | 'offline' | 'generic'

/** Traduz a mensagem crua do backend (`client.rs`/`servers.rs`) numa dica de correção. */
export function errorHint(message: string, command: string): McpErrorHint {
  if (/not on PATH|not found|ENOENT|No such file/i.test(message)) {
    const bin =
      command
        .trim()
        .replace(/\.(cmd|exe)$/i, '')
        .split(/[\\/]/)
        .pop() ?? ''
    if (['npx', 'node', 'npm'].includes(bin)) return 'notFoundNode'
    if (['uvx', 'uv'].includes(bin)) return 'notFoundUv'
    return 'notFound'
  }
  if (/no answer in|timed? ?out/i.test(message)) return 'timeout'
  if (/\benv\b/i.test(message)) return 'env'
  if (/offline/i.test(message)) return 'offline'
  return 'generic'
}
