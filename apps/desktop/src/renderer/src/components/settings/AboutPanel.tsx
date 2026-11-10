import { LEGAL_DOCUMENT_IDS, type LegalDocumentId } from '@mika/sdk/contracts'
import type { TranslationDict } from '@mika/sdk/i18n'
import { Boxes, FileText, Info, type LucideIcon, Package, Scale, ShieldCheck, X } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { type LegalDocState, useLegalDocument } from '../../hooks/useLegalDocument'
import { rendererLog } from '../../lib/renderer-logger'
import { getElectronApi } from '../../lib/shell-api'
import { BentoGrid, GlassCard, GlassSection, PageHero, StatTile, StatusPill } from './ds'

type DocEntry = {
  id: LegalDocumentId
  icon: LucideIcon
  title: keyof TranslationDict
  description: keyof TranslationDict
}

const META: Record<LegalDocumentId, Omit<DocEntry, 'id'>> = {
  eula: { icon: Scale, title: 'settings.about.doc.eula.title', description: 'settings.about.doc.eula.description' },
  privacy: {
    icon: ShieldCheck,
    title: 'settings.about.doc.privacy.title',
    description: 'settings.about.doc.privacy.description',
  },
  notices: {
    icon: Package,
    title: 'settings.about.doc.notices.title',
    description: 'settings.about.doc.notices.description',
  },
  assets: {
    icon: Boxes,
    title: 'settings.about.doc.assets.title',
    description: 'settings.about.doc.assets.description',
  },
  license: {
    icon: FileText,
    title: 'settings.about.doc.license.title',
    description: 'settings.about.doc.license.description',
  },
}

/** Ordem do contrato; `Record<LegalDocumentId, …>` não compila se um id novo ficar sem texto. */
const DOCS: readonly DocEntry[] = LEGAL_DOCUMENT_IDS.map((id) => ({ id, ...META[id] }))

type Build = { version: string; packaged: boolean }

function useBuild(): Build | null {
  const [build, setBuild] = useState<Build | null>(null)
  useEffect(() => {
    let mounted = true
    getElectronApi()
      .runtime.getHealth()
      .then((health) => {
        if (mounted) setBuild({ version: health.version, packaged: health.packaged })
      })
      .catch((error) => rendererLog.warn('about: health unavailable', { error }))
    return () => {
      mounted = false
    }
  }, [])
  return build
}

function DocCard({ entry, active, onRead }: { entry: DocEntry; active: boolean; onRead: () => void }) {
  const { t } = useI18n()
  const Icon = entry.icon
  return (
    <GlassCard hover={false}>
      <div className="flex h-full flex-col gap-3">
        <div className="flex items-start gap-3">
          <span
            aria-hidden="true"
            className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent"
          >
            <Icon size={16} />
          </span>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold text-text-main">{t(entry.title)}</h3>
            <p className="mt-0.5 text-xs leading-relaxed text-text-muted">{t(entry.description)}</p>
          </div>
        </div>
        <button
          type="button"
          aria-pressed={active}
          onClick={onRead}
          className="mt-auto inline-flex items-center justify-center gap-1.5 rounded-xl border border-accent-rim bg-accent-soft px-3 py-2 text-xs font-bold text-accent-text transition hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-[0.98]"
        >
          {t('settings.about.read')}
        </button>
      </div>
    </GlassCard>
  )
}

function Reader({ entry, state, onClose }: { entry: DocEntry; state: LegalDocState; onClose: () => void }) {
  const { t } = useI18n()
  return (
    <GlassSection title={t(entry.title)} icon={entry.icon} description={t(entry.description)}>
      <div className="mb-3 flex justify-end">
        <button
          type="button"
          onClick={onClose}
          className="inline-flex items-center gap-1.5 rounded-lg border border-border-subtle px-3 py-1.5 text-caption font-bold text-text-main transition hover:bg-surface-raise focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus active:scale-95"
        >
          <X size={12} aria-hidden="true" />
          {t('button.close')}
        </button>
      </div>
      {state.status === 'loading' && (
        <p role="status" className="text-caption text-text-muted">
          {t('status.loading')}
        </p>
      )}
      {state.status === 'error' && (
        <div
          role="alert"
          className="rounded-lg border border-danger-rim bg-danger-soft px-3 py-2 text-caption text-danger"
        >
          {t('settings.about.loadError')}
        </div>
      )}
      {state.status === 'ready' && (
        <section
          // biome-ignore lint/a11y/noNoninteractiveTabindex: a scroll area must take focus so the keyboard can scroll a long legal text
          tabIndex={0}
          aria-label={t('settings.about.reader.aria')}
          className="max-h-[60vh] overflow-auto rounded-xl border border-border-subtle bg-surface-raise p-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        >
          <pre className="whitespace-pre-wrap break-words font-mono text-caption leading-relaxed text-text-main">
            {state.text}
          </pre>
        </section>
      )}
    </GlassSection>
  )
}

export function AboutPanel() {
  const { t } = useI18n()
  const build = useBuild()
  const [openDoc, setOpenDoc] = useState<LegalDocumentId | null>(null)
  const state = useLegalDocument(openDoc)
  const active = DOCS.find((doc) => doc.id === openDoc)
  const buildKind = build?.packaged ? t('settings.update.facts.packaged') : t('settings.update.facts.development')

  return (
    <div className="space-y-6">
      <PageHero
        icon={Info}
        title={t('settings.about.title')}
        description={t('settings.about.description')}
        pills={
          <>
            <StatusPill variant="warn" label={t('settings.about.draft')} dot />
            <StatusPill variant="neutral" label={t('settings.about.aiLive')} />
            {build && <StatusPill variant="neutral" label={buildKind} />}
          </>
        }
        aside={
          <StatTile
            label={t('settings.update.facts.installedVersion')}
            value={build ? `v${build.version}` : t('settings.update.facts.checking')}
          />
        }
      />
      <GlassSection
        title={t('settings.about.docs.title')}
        icon={Scale}
        description={t('settings.about.docs.description')}
      >
        <p className="mb-3 text-caption leading-relaxed text-text-muted">{t('settings.about.language')}</p>
        <p className="mb-3 text-caption leading-relaxed text-text-muted">{t('legal.consent.aiNotice')}</p>
        <BentoGrid cols={2}>
          {DOCS.map((entry) => (
            <DocCard key={entry.id} entry={entry} active={openDoc === entry.id} onRead={() => setOpenDoc(entry.id)} />
          ))}
        </BentoGrid>
      </GlassSection>
      {active && <Reader entry={active} state={state} onClose={() => setOpenDoc(null)} />}
    </div>
  )
}
