import { AnimatePresence, motion } from 'framer-motion'
import { Check } from 'lucide-react'
import type { ComponentType, ReactNode } from 'react'
import { useId } from 'react'
import { MOTION } from '../motion'
import { cn } from '../utils/cn'
import { type PillVariant, StatusPill } from './StatusPill'

export interface ProviderBadge {
  label: string
  variant: PillVariant
}

export interface ProviderChoice {
  id: string
  label: string
  desc: string
  icon?: ComponentType<{ size?: number; strokeWidth?: number; className?: string }>
  badges?: readonly ProviderBadge[]
}

/** Mapeia a tecla de seta para o próximo item na grade circular, ou null se a tecla for irrelevante. */
export function nextProviderForKey<T extends ProviderChoice>(
  key: string,
  providers: readonly T[],
  activeId: string,
): T | null {
  const idx = providers.findIndex((p) => p.id === activeId)
  if (idx === -1) return null
  if (key === 'ArrowRight' || key === 'ArrowDown') return providers[(idx + 1) % providers.length] ?? null
  if (key === 'ArrowLeft' || key === 'ArrowUp')
    return providers[(idx - 1 + providers.length) % providers.length] ?? null
  return null
}

interface ProviderChoiceGridProps<T extends ProviderChoice> {
  providers: readonly T[]
  activeId: string

  columnsClassName?: string
  disabled?: boolean
  renderActiveIndicator?: () => ReactNode
  /** Detalhes do provedor ativo, abertos num painel logo abaixo da grade. */
  renderDetails?: (provider: T) => ReactNode
  onSelect: (provider: T) => void
}

function ProviderIcon({ provider, active }: { provider: ProviderChoice; active: boolean }) {
  const Icon = provider.icon
  if (!Icon) return null
  return (
    <span
      aria-hidden="true"
      className={cn(
        'flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border transition-colors',
        active ? 'border-accent-rim bg-accent-soft text-accent' : 'border-border-subtle bg-bg-surface text-text-muted',
      )}
    >
      <Icon size={18} strokeWidth={2} />
    </span>
  )
}

/** Mesmo selo de check dos cards de escolha (GlassChoiceCards); `indicator` substitui só no ativo. */
function ActiveMark({ active, indicator }: { active: boolean; indicator?: () => ReactNode }) {
  if (active && indicator)
    return <span className="mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center">{indicator()}</span>
  return (
    <span
      aria-hidden="true"
      className={cn(
        'mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition duration-200',
        active ? 'border-accent bg-accent text-bg-deep' : 'border-border-subtle text-transparent',
      )}
    >
      <Check size={11} strokeWidth={3} />
    </span>
  )
}

function Badges({ badges }: { badges?: readonly ProviderBadge[] }) {
  if (!badges?.length) return null
  return (
    <div className="mt-2 flex flex-wrap gap-1">
      {badges.map((badge) => (
        <StatusPill key={badge.label} variant={badge.variant} label={badge.label} />
      ))}
    </div>
  )
}

function ProviderDetails<T extends ProviderChoice>({ provider, children }: { provider: T; children: ReactNode }) {
  const titleId = useId()
  return (
    <motion.section
      key={provider.id}
      aria-labelledby={titleId}
      {...MOTION.popover}
      className="mt-3 rounded-xl border border-accent-rim bg-surface-raise/60 p-4 shadow-[0_0_24px_var(--accent-glow)]"
    >
      <header className="mb-3 flex items-start gap-3 border-b border-border-subtle pb-3">
        <ProviderIcon provider={provider} active />
        <div className="min-w-0 flex-1">
          <h4 id={titleId} className="text-sm font-bold text-text-main">
            {provider.label}
          </h4>
          <p className="text-caption text-text-muted">{provider.desc}</p>
        </div>
      </header>
      <div className="flex flex-col gap-4">{children}</div>
    </motion.section>
  )
}

/** Colunas pela largura do container e pela quantidade: 2 lado a lado, 4 em 2×2/4, resto em 3. */
function defaultColumns(count: number): string {
  if (count <= 2) return 'grid-cols-1 @md:grid-cols-2'
  if (count === 4) return 'grid-cols-1 @md:grid-cols-2 @5xl:grid-cols-4'
  return 'grid-cols-1 @md:grid-cols-2 @3xl:grid-cols-3'
}

export function ProviderChoiceGrid<T extends ProviderChoice>({
  providers,
  activeId,
  columnsClassName,
  disabled = false,
  renderActiveIndicator,
  renderDetails,
  onSelect,
}: ProviderChoiceGridProps<T>) {
  const active = providers.find((p) => p.id === activeId)
  const details = active && renderDetails ? renderDetails(active) : null
  return (
    <div className="@container">
      <div className={cn('grid gap-2', columnsClassName ?? defaultColumns(providers.length))}>
        {providers.map((provider) => {
          const isActive = activeId === provider.id
          return (
            <button
              key={provider.id}
              type="button"
              aria-pressed={isActive}
              onClick={() => onSelect(provider)}
              onKeyDown={(e) => {
                if (disabled) return
                const next = nextProviderForKey(e.key, providers, provider.id)
                if (!next) return
                e.preventDefault()
                onSelect(next)
              }}
              disabled={disabled}
              className={cn(
                'relative flex flex-col overflow-hidden rounded-xl border p-3 text-left transition duration-200 disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus',
                isActive
                  ? 'border-accent-rim bg-accent-soft shadow-[0_0_18px_var(--accent-glow)]'
                  : 'border-border-subtle bg-surface-raise enabled:hover:-translate-y-0.5 enabled:hover:border-border-strong enabled:hover:bg-surface-raise-strong',
              )}
            >
              {isActive && <span aria-hidden="true" className="absolute inset-x-0 top-0 h-0.5 bg-accent" />}
              <div className="flex items-start gap-2.5">
                <ProviderIcon provider={provider} active={isActive} />
                <div className="min-w-0 flex-1">
                  <div
                    className={cn(
                      'break-words text-sm font-bold leading-tight',
                      isActive ? 'text-accent-text' : 'text-text-main',
                    )}
                  >
                    {provider.label}
                  </div>
                  <div className="mt-1 text-caption leading-snug text-text-muted">{provider.desc}</div>
                </div>
                <ActiveMark active={isActive} indicator={renderActiveIndicator} />
              </div>
              <Badges badges={provider.badges} />
            </button>
          )
        })}
      </div>
      <AnimatePresence mode="wait" initial={false}>
        {active && details ? (
          <ProviderDetails key={active.id} provider={active}>
            {details}
          </ProviderDetails>
        ) : null}
      </AnimatePresence>
    </div>
  )
}
