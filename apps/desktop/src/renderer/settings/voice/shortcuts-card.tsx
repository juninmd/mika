import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassCard, SectionHeader, StatusPill } from '@mika/sdk-ui/settings'
import { Globe, Keyboard, PanelTop } from 'lucide-react'
import type { ComponentType } from 'react'
import { type AppShortcut, formatAccelerator, GLOBAL_SHORTCUTS, WINDOW_SHORTCUTS } from '../../src/lib/app-shortcuts'
import { isMacPlatform } from '../../src/lib/keyboard-shortcuts'

type T = (key: keyof TranslationDict) => string

export function Keys({ accelerator, mac }: { accelerator: string; mac: boolean }) {
  return (
    <span className="flex shrink-0 items-center gap-1">
      {formatAccelerator(accelerator, mac)
        .split('+')
        .map((part) => (
          <kbd
            key={part}
            className="min-w-7 rounded-md border border-border-strong bg-bg-deep px-1.5 py-0.5 text-center font-mono text-caption font-bold text-text-main shadow-[inset_0_-2px_0_var(--border-subtle)]"
          >
            {part}
          </kbd>
        ))}
    </span>
  )
}

function ShortcutGroup({
  t,
  title,
  icon: Icon,
  items,
  mac,
}: {
  t: T
  title: string
  icon: ComponentType<{ size?: number; className?: string }>
  items: readonly AppShortcut[]
  mac: boolean
}) {
  return (
    <div className="rounded-xl border border-border-subtle bg-surface-raise/40 p-3">
      <p className="mb-2 flex items-center gap-1.5 text-micro font-black uppercase tracking-[0.12em] text-text-muted">
        <Icon size={12} aria-hidden="true" />
        {title}
      </p>
      <ul className="divide-y divide-border-subtle">
        {items.map((item) => (
          <li key={item.id} className="flex items-center gap-3 py-2">
            <Keys accelerator={item.accelerator} mac={mac} />
            <div className="min-w-0 flex-1">
              <p className="flex flex-wrap items-center gap-1.5 text-sm font-semibold text-text-main">
                {t(item.label)}
                {item.hold && <StatusPill variant="accent" label={t('settings.shortcuts.holdBadge')} />}
              </p>
              <p className="text-caption leading-snug text-text-muted">{t(item.description)}</p>
            </div>
          </li>
        ))}
      </ul>
    </div>
  )
}

/** Every real shortcut, read from `app-shortcuts.ts` — the list `shortcuts.rs` is tested against. */
export function ShortcutsCard({ t }: { t: T }) {
  const mac = isMacPlatform()
  return (
    <GlassCard span="full">
      <SectionHeader
        title={t('settings.shortcuts.title')}
        description={t('settings.shortcuts.description')}
        actions={<Keyboard size={16} className="text-text-muted" aria-hidden="true" />}
        className="mb-3"
      />
      <div className="grid gap-3 lg:grid-cols-2">
        <ShortcutGroup t={t} title={t('settings.shortcuts.global')} icon={Globe} items={GLOBAL_SHORTCUTS} mac={mac} />
        <ShortcutGroup
          t={t}
          title={t('settings.shortcuts.window')}
          icon={PanelTop}
          items={WINDOW_SHORTCUTS}
          mac={mac}
        />
      </div>
    </GlassCard>
  )
}
