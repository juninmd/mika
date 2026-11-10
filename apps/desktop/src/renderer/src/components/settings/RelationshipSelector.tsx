import { Check, Gamepad2, Handshake, Heart, type LucideIcon, Users } from 'lucide-react'
import { useCallback, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { useUIStore } from '../../store/useUIStore'
import { GlassSlider, SectionHeader } from './ds'

export type Relationship = 'acquaintance' | 'friend' | 'close-friend' | 'companion'

/** Mapeia a tecla de seta para o próximo id na lista circular, ou null se a tecla for irrelevante. */
export function nextRelationshipIdForKey<T extends { id: Relationship }>(
  key: string,
  options: T[],
  currentId: Relationship,
): Relationship | null {
  const idx = options.findIndex((o) => o.id === currentId)
  if (idx === -1) return null
  if (key === 'ArrowRight' || key === 'ArrowDown') {
    return options[(idx + 1) % options.length]?.id ?? null
  }
  if (key === 'ArrowLeft' || key === 'ArrowUp') {
    return options[(idx - 1 + options.length) % options.length]?.id ?? null
  }
  return null
}

type Props = {
  value: Relationship
  affinityScore: number
  onSave: (path: string, value: unknown) => Promise<void>
}

function useRelationshipOptions(): {
  id: Relationship
  label: string
  desc: string
  icon: LucideIcon
}[] {
  const { t } = useI18n()
  return [
    {
      id: 'acquaintance',
      label: t('settings.relationship.acquaintanceLabel'),
      desc: t('settings.relationship.acquaintanceDesc'),
      icon: Handshake,
    },
    {
      id: 'friend',
      label: t('settings.relationship.friendLabel'),
      desc: t('settings.relationship.friendDesc'),
      icon: Gamepad2,
    },
    {
      id: 'close-friend',
      label: t('settings.relationship.closeFriendLabel'),
      desc: t('settings.relationship.closeFriendDesc'),
      icon: Users,
    },
    {
      id: 'companion',
      label: t('settings.relationship.companionLabel'),
      desc: t('settings.relationship.companionDesc'),
      icon: Heart,
    },
  ]
}

export function RelationshipSelector({ value, affinityScore, onSave }: Props) {
  const { t } = useI18n()
  const OPTIONS = useRelationshipOptions()
  const [saving, setSaving] = useState(false)

  const doSave = useCallback(
    async (v: Relationship) => {
      setSaving(true)
      try {
        await onSave('character.relationshipStatus', v)
      } catch {
        useUIStore.getState().addNotification('error', t('settings.relationship.saveError'))
      } finally {
        setSaving(false)
      }
    },
    [onSave, t],
  )

  return (
    // Rendered inside a GlassCard: a second rounded frame and icon tile read as a card within a card.
    <div>
      <SectionHeader
        title={t('settings.relationship.title')}
        description={t('settings.relationship.subtitle')}
        className="mb-4"
      />
      <div className="grid gap-3 sm:grid-cols-2">
        {OPTIONS.map((opt) => {
          const active = value === opt.id
          const Icon = opt.icon
          return (
            <button
              key={opt.id}
              type="button"
              aria-pressed={active}
              disabled={saving}
              onClick={() => doSave(opt.id)}
              onKeyDown={(e) => {
                if (saving) return
                const nextId = nextRelationshipIdForKey(e.key, OPTIONS, opt.id)
                if (!nextId) return
                e.preventDefault()
                doSave(nextId)
              }}
              className={`relative overflow-hidden rounded-xl border p-3 text-left transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed ${
                active
                  ? 'border-accent-rim bg-accent-soft shadow-[0_0_18px_var(--accent-glow)]'
                  : 'border-border-subtle bg-surface-raise enabled:hover:-translate-y-0.5 enabled:hover:border-border-strong enabled:hover:bg-surface-raise-strong'
              }`}
            >
              {active && <span aria-hidden="true" className="absolute inset-x-0 top-0 h-0.5 bg-accent" />}
              <div className="flex items-start gap-3">
                <span
                  aria-hidden="true"
                  className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border ${
                    active
                      ? 'border-accent-rim bg-accent-soft text-accent'
                      : 'border-border-subtle bg-bg-surface text-text-muted'
                  }`}
                >
                  <Icon size={17} />
                </span>
                <div className="min-w-0 flex-1 self-center">
                  <div
                    className={`text-sm font-semibold leading-tight ${active ? 'text-accent-text' : 'text-text-main'}`}
                  >
                    {opt.label}
                  </div>
                  <div className="mt-1 text-caption leading-snug text-text-muted">{opt.desc}</div>
                </div>
                <span
                  aria-hidden="true"
                  className={`mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border transition duration-200 ${
                    active ? 'border-accent bg-accent text-bg-deep' : 'border-border-subtle text-transparent'
                  } ${active && saving ? 'animate-pulse' : ''}`}
                >
                  <Check size={11} strokeWidth={3} />
                </span>
              </div>
            </button>
          )
        })}
      </div>

      <div className="mt-5">
        <GlassSlider
          label={t('settings.relationship.affinityLabel')}
          configPath="character.affinityScore"
          value={affinityScore}
          min={-100}
          max={100}
          minLabel={t('settings.relationship.affinityMinLabel')}
          maxLabel={t('settings.relationship.affinityMaxLabel')}
          onSave={onSave}
        />
      </div>
    </div>
  )
}
