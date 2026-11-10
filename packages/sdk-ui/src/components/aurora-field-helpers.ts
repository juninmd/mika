import { getByPath } from '../utils/get-by-path'

export interface AuroraFieldProps {
  label: string
  description?: string
  /** Modo ligado à config: grava no blur/Enter. Omita os três e use `value`/`onChange` num formulário. */
  configPath?: string
  config?: Record<string, unknown>
  onSave?: (path: string, value: unknown) => Promise<void>
  /** Modo controlado: o dono do formulário guarda o valor e decide quando salvar. */
  value?: string
  onChange?: (value: string) => void
  type?: 'text' | 'number' | 'password' | 'url' | 'email' | 'time' | 'search' | 'textarea'
  placeholder?: string
  badge?: string
  accentHue?: number
  rows?: number
  disabled?: boolean
  warning?: string
  /** Enter (fora de textarea) chama isto em vez de salvar — formulário de "adicionar", busca que dispara. */
  onSubmit?: () => void
  /** Ação colada à direita do campo (botão de adicionar, copiar, testar). */
  trailing?: React.ReactNode
  /** Rótulo só para leitor de tela — caixas de busca em que o placeholder já diz o que é. */
  hideLabel?: boolean
  /** Modo controlado: o usuário terminou de editar (blur, ou Enter sem `onSubmit`) — hora de salvar. */
  onCommit?: () => void
  /** Id de texto externo que descreve o campo (erro/ajuda renderizados fora do controle). */
  describedBy?: string
  /** Valor recusado pelo dono do formulário — o `warning` passa a ser o erro lido pelo leitor de tela. */
  invalid?: boolean
}

export function buildInputClass(focused: boolean) {
  const base =
    'w-full rounded-lg border bg-bg-surface px-3 py-2 text-[13px] text-text-main outline-none transition duration-150 placeholder:text-text-muted/60'
  if (focused) return `${base} border-border-subtle`
  return `${base} border-border-subtle hover:border-border-strong`
}

export function readInitial({ config, configPath, value, onChange }: AuroraFieldProps): string {
  const raw = onChange !== undefined || !config || !configPath ? value : getByPath(config, configPath)
  return raw != null ? String(raw) : ''
}

/** Draft of a number field → value to save, or `null` when there is none (blank, non-finite). */
export function parseNumberDraft(draft: string): number | null {
  if (draft.trim() === '') return null
  const value = Number(draft)
  return Number.isFinite(value) ? value : null
}

export const INPUT_PADDING: Partial<Record<NonNullable<AuroraFieldProps['type']>, string>> = {
  password: 'pr-9',
  search: 'pl-8',
}

export function buildWrapperClass(disabled: boolean) {
  return disabled ? 'space-y-1.5 opacity-50' : 'space-y-1.5'
}
