import type { McpServersListResponse } from '@mika/sdk/contracts'
import { useCopyToClipboard } from '@mika/sdk-ui'
import { MOTION } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import {
  Check,
  CloudOff,
  Copy,
  FileBraces,
  FolderOpen,
  type LucideIcon,
  Plus,
  PowerOff,
  RefreshCw,
  Server,
  TriangleAlert,
} from 'lucide-react'
import type { ReactNode } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { BTN } from './mcp-ui'

type Tone = 'warn' | 'danger' | 'info'

const TONE: Record<Tone, string> = {
  warn: 'border-warn-rim bg-warn-soft text-warn',
  danger: 'border-danger-rim bg-danger-soft text-danger',
  info: 'border-info-rim bg-info-soft text-info',
}

function Banner({
  tone,
  icon: Icon,
  title,
  children,
  action,
}: {
  tone: Tone
  icon: LucideIcon
  title: string
  children: ReactNode
  action?: ReactNode
}) {
  return (
    <motion.div
      {...MOTION.fadeUp}
      role={tone === 'danger' ? 'alert' : 'status'}
      className={`flex flex-wrap items-start gap-3 rounded-2xl border px-4 py-3.5 ${TONE[tone]}`}
    >
      <Icon size={18} className="mt-0.5 shrink-0" aria-hidden="true" />
      <div className="min-w-0 flex-1 space-y-1">
        <p className="text-sm font-bold">{title}</p>
        <div className="text-caption leading-relaxed text-text-muted">{children}</div>
      </div>
      {action}
    </motion.div>
  )
}

/** Por que nenhum servidor sobe: JSON inválido, MCP desligado ou modo offline. */
export function McpBanners({ data, onOpenDir }: { data: McpServersListResponse; onOpenDir: () => void }) {
  const { t } = useI18n()
  return (
    <>
      {data.error !== null && (
        <Banner
          tone="danger"
          icon={FileBraces}
          title={t('settings.mcp.banner.invalidTitle')}
          action={
            <button type="button" className={BTN.ghost} onClick={onOpenDir}>
              <FolderOpen size={13} aria-hidden="true" />
              {t('settings.mcp.advanced.openDir')}
            </button>
          }
        >
          <p>{t('settings.mcp.banner.invalidBody')}</p>
          <code className="mt-1.5 block break-words font-mono text-caption text-danger">{data.error}</code>
        </Banner>
      )}
      {data.offlineMode && (
        <Banner tone="warn" icon={CloudOff} title={t('settings.mcp.banner.offlineTitle')}>
          {t('settings.mcp.banner.offlineBody')}
        </Banner>
      )}
      {!data.enabled && !data.offlineMode && (
        <Banner tone="info" icon={PowerOff} title={t('settings.mcp.banner.disabledTitle')}>
          {t('settings.mcp.banner.disabledBody')}
        </Banner>
      )}
    </>
  )
}

const STEPS = ['pick', 'fill', 'test'] as const

export function McpEmptyState({ onAdd, disabled }: { onAdd: () => void; disabled: boolean }) {
  const { t } = useI18n()
  return (
    <motion.div
      {...MOTION.fadeUp}
      className="relative overflow-hidden rounded-2xl border border-dashed border-accent-rim bg-accent-soft/20 px-6 py-8"
    >
      <div className="flex flex-col items-center gap-2 text-center">
        <span
          aria-hidden="true"
          className="flex h-14 w-14 items-center justify-center rounded-2xl border border-accent-rim bg-accent-soft text-accent shadow-[0_0_28px_var(--accent-glow)]"
        >
          <Server size={24} />
        </span>
        <h4 className="mt-2 text-base font-bold text-text-main">{t('settings.mcp.empty.title')}</h4>
        <p className="max-w-lg text-caption leading-relaxed text-text-muted">{t('settings.mcp.empty.body')}</p>
      </div>
      <ol className="mx-auto mt-6 grid max-w-3xl gap-3 sm:grid-cols-3">
        {STEPS.map((step, i) => (
          <li key={step} className="rounded-xl border border-border-subtle bg-bg-deep/50 p-3.5">
            <span className="flex h-6 w-6 items-center justify-center rounded-full border border-accent-rim bg-accent-soft text-caption font-bold text-accent">
              {i + 1}
            </span>
            <p className="mt-2 text-caption font-bold text-text-main">{t(`settings.mcp.empty.step.${step}.title`)}</p>
            <p className="mt-0.5 text-caption leading-snug text-text-muted">
              {t(`settings.mcp.empty.step.${step}.body`)}
            </p>
          </li>
        ))}
      </ol>
      <div className="mt-6 flex justify-center">
        <button type="button" className={BTN.hero} onClick={onAdd} disabled={disabled}>
          <Plus size={16} aria-hidden="true" />
          {t('settings.mcp.servers.add')}
        </button>
      </div>
    </motion.div>
  )
}

export function McpSkeleton() {
  const { t } = useI18n()
  return (
    <div role="status" aria-label={t('settings.mcp.servers.loading')} className="space-y-3">
      {[0, 1].map((i) => (
        <div
          key={i}
          className="flex items-center gap-4 rounded-2xl border border-border-subtle bg-surface-raise/30 p-4"
        >
          <div className="h-11 w-11 animate-pulse rounded-xl bg-surface-raise" />
          <div className="flex-1 space-y-2">
            <div className="h-3.5 w-40 animate-pulse rounded bg-surface-raise" />
            <div className="h-3 w-72 max-w-full animate-pulse rounded bg-surface-raise/70" />
          </div>
          <div className="h-6 w-11 animate-pulse rounded-full bg-surface-raise" />
        </div>
      ))}
    </div>
  )
}

export function McpLoadFailed({ error, onRetry }: { error: string; onRetry: () => void }) {
  const { t } = useI18n()
  return (
    <Banner
      tone="danger"
      icon={TriangleAlert}
      title={t('settings.mcp.servers.loadFailed')}
      action={
        <button type="button" className={BTN.ghost} onClick={onRetry}>
          <RefreshCw size={13} aria-hidden="true" />
          {t('button.retry')}
        </button>
      }
    >
      <code className="break-words font-mono">{error}</code>
    </Banner>
  )
}

export function ConfigPathRow({ path, onOpenDir }: { path: string; onOpenDir: () => void }) {
  const { t } = useI18n()
  const { copied, copy } = useCopyToClipboard()
  return (
    <div className="space-y-2">
      <p className="text-caption font-semibold text-text-main">{t('settings.mcp.advanced.pathLabel')}</p>
      <div className="flex flex-wrap items-center gap-2">
        <code
          title={path}
          className="min-w-0 flex-1 select-all truncate rounded-lg border border-border-subtle bg-bg-deep/60 px-3 py-2 font-mono text-caption text-text-main"
        >
          {path}
        </code>
        <button
          type="button"
          className={BTN.icon}
          onClick={() => void copy(path).catch(() => {})}
          aria-label={copied ? t('button.copied') : t('button.copy')}
          title={copied ? t('button.copied') : t('button.copy')}
        >
          {copied ? <Check size={14} className="text-ok" aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
        </button>
        <button type="button" className={BTN.ghost} onClick={onOpenDir}>
          <FolderOpen size={13} aria-hidden="true" />
          {t('settings.mcp.advanced.openDir')}
        </button>
      </div>
      <p className="text-caption leading-relaxed text-text-subtle">{t('settings.mcp.advanced.pathHint')}</p>
    </div>
  )
}
