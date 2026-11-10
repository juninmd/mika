import type { McpServerSummary } from '@mika/sdk/contracts'
import { DURATION, EASE, PRESSABLE, StatusPill, staggerDelay } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import {
  BrainCircuit,
  Check,
  FolderTree,
  GitPullRequest,
  Globe,
  type LucideIcon,
  MonitorPlay,
  Plus,
  Search,
} from 'lucide-react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { commandPreview } from './mcp-server-form'
import { MCP_TEMPLATES, type McpTemplate, templateInstalled } from './mcp-templates'

const ICONS: Record<McpTemplate['id'], LucideIcon> = {
  filesystem: FolderTree,
  github: GitPullRequest,
  'brave-search': Search,
  fetch: Globe,
  memory: BrainCircuit,
  playwright: MonitorPlay,
}

type GalleryProps = {
  servers: readonly McpServerSummary[]
  disabled: boolean
  onPick: (template: McpTemplate) => void
}

export function McpTemplatesGallery({ servers, disabled, onPick }: GalleryProps) {
  const { t } = useI18n()
  return (
    <ul className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
      {MCP_TEMPLATES.map((template, i) => {
        const Icon = ICONS[template.id]
        const installed = templateInstalled(template, servers)
        return (
          <motion.li
            key={template.id}
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: DURATION.base, ease: EASE.out, delay: staggerDelay(i) }}
          >
            <motion.button
              type="button"
              {...PRESSABLE}
              disabled={disabled}
              onClick={() => onPick(template)}
              aria-label={t('settings.mcp.template.use').replace('{{name}}', template.brand)}
              className="group flex h-full w-full flex-col gap-3 rounded-2xl border border-border-subtle bg-surface-raise/30 p-4 text-left transition-colors hover:border-accent-rim hover:bg-accent-soft/40 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
            >
              <div className="flex items-center gap-3">
                <span
                  aria-hidden="true"
                  className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-accent-rim bg-accent-soft text-accent transition-shadow group-hover:shadow-[0_0_18px_var(--accent-glow)]"
                >
                  <Icon size={18} />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-bold text-text-main">{template.brand}</span>
                  <span className="block text-micro font-bold uppercase tracking-[0.12em] text-text-subtle">
                    {t(`settings.mcp.template.need.${template.need}`)}
                  </span>
                </span>
                {installed ? (
                  <StatusPill variant="ok" label={t('settings.mcp.template.installed')} />
                ) : (
                  <Plus
                    size={16}
                    aria-hidden="true"
                    className="text-text-subtle transition-colors group-hover:text-accent"
                  />
                )}
              </div>
              <span className="flex-1 text-caption leading-relaxed text-text-muted">
                {t(`settings.mcp.template.desc.${template.id}`)}
              </span>
              <code className="block truncate rounded-md border border-border-subtle bg-bg-deep/60 px-2 py-1 font-mono text-micro text-text-muted">
                {commandPreview(template.command, template.args)}
              </code>
              <span className="inline-flex items-center gap-1 text-caption font-bold text-accent opacity-80 transition-opacity group-hover:opacity-100">
                {installed ? <Check size={12} aria-hidden="true" /> : <Plus size={12} aria-hidden="true" />}
                {t(installed ? 'settings.mcp.template.addAnother' : 'settings.mcp.template.add')}
              </span>
            </motion.button>
          </motion.li>
        )
      })}
    </ul>
  )
}
