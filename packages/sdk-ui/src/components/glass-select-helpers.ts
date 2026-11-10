import { cn } from '../utils/cn'
import { getByPath } from '../utils/get-by-path'

const PANEL_GAP = 6

const VIEWPORT_MARGIN = 8

export const LIST_MAX_HEIGHT = 240

export const ESTIMATED_PANEL_HEIGHT = 288

const MIN_LIST_HEIGHT = 120

export interface GlassSelectOption {
  label: string

  value: string

  description?: string

  icon?: React.ReactNode
}

export interface GlassSelectProps {
  label: string

  description?: string

  /** Modo ligado à config. Omita `configPath`/`config`/`onSave` e use `value`/`onChange` num formulário. */
  configPath?: string

  options: GlassSelectOption[]

  config?: Record<string, unknown>

  onSave?: (path: string, value: string) => Promise<void>

  value?: string

  onChange?: (value: string) => void

  /** Rótulo só para leitor de tela — filtros compactos numa barra de ferramentas. */
  hideLabel?: boolean

  /** Id de texto externo que descreve o campo (erro/ajuda renderizados fora do controle). */
  describedBy?: string

  searchable?: boolean

  accentHue?: number

  disabled?: boolean

  /** `auto` (padrão): até {@link CARD_MAX_OPTIONS} opções viram cards; lista longa fica dropdown. */
  variant?: 'auto' | 'cards' | 'dropdown'
}

export const CARD_MAX_OPTIONS = 6

/**
 * Uma escolha curta como cards: todas as opções e suas descrições visíveis de uma vez, sem abrir
 * nada. Lista longa, pesquisável ou compacta de barra de ferramentas continua dropdown.
 */
export function rendersAsCards({ variant = 'auto', options, searchable, hideLabel }: GlassSelectProps): boolean {
  if (variant !== 'auto') return variant === 'cards'
  return !searchable && !hideLabel && options.length >= 2 && options.length <= CARD_MAX_OPTIONS
}

export type PanelPosition = { top: number; left: number; width: number; listMaxHeight: number }

export function computePosition(rect: DOMRect, panelHeight: number): PanelPosition {
  const spaceBelow = window.innerHeight - rect.bottom - PANEL_GAP - VIEWPORT_MARGIN
  const spaceAbove = rect.top - PANEL_GAP - VIEWPORT_MARGIN
  const flipUp = panelHeight > spaceBelow && spaceAbove > spaceBelow
  const available = Math.max(MIN_LIST_HEIGHT, flipUp ? spaceAbove : spaceBelow)
  const maxLeft = Math.max(VIEWPORT_MARGIN, window.innerWidth - rect.width - VIEWPORT_MARGIN)
  const left = Math.max(VIEWPORT_MARGIN, Math.min(rect.left, maxLeft))
  const width = Math.min(rect.width, window.innerWidth - VIEWPORT_MARGIN * 2)
  const top = flipUp
    ? Math.max(VIEWPORT_MARGIN, rect.top - PANEL_GAP - Math.min(panelHeight, available))
    : Math.min(rect.bottom + PANEL_GAP, window.innerHeight - Math.min(panelHeight, available) - VIEWPORT_MARGIN)

  return {
    left,
    width,
    top,
    listMaxHeight: Math.min(LIST_MAX_HEIGHT, Math.max(MIN_LIST_HEIGHT, available - 50)),
  }
}

export function readCurrentValue({ config, configPath, value }: GlassSelectProps): string {
  const raw = config && configPath ? getByPath(config, configPath) : value
  return raw != null ? String(raw) : ''
}

export function containerClass(disabled: boolean): string {
  return cn('flex flex-col gap-2', disabled && 'opacity-50')
}

export function triggerClass(isOpen: boolean): string {
  return cn(
    'flex w-full items-center justify-between rounded-lg border border-border-subtle bg-surface-raise px-3 py-2 text-[13px] text-text-main transition duration-200 hover:border-border-strong hover:bg-surface-raise-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]/60 disabled:cursor-not-allowed',
    isOpen && 'border-[var(--accent)]/50 ring-2 ring-[var(--accent)]/40 shadow-[0_0_20px_var(--accent-glow)]',
  )
}

export function nextIndexFor(key: string, current: number, lastIndex: number): number | null {
  if (key === 'ArrowDown') return current >= lastIndex ? 0 : current + 1
  if (key === 'ArrowUp') return current <= 0 ? lastIndex : current - 1
  if (key === 'Home') return 0
  if (key === 'End') return lastIndex
  return null
}

/** Mesma regra de filtro do painel: rótulo contém o texto de busca, sem diferenciar maiúsculas. */
export function filterOptions(options: GlassSelectOption[], search: string): GlassSelectOption[] {
  const query = search.toLowerCase()
  return options.filter((o) => o.label.toLowerCase().includes(query))
}
