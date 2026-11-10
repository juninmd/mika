import type { McpServerSummary } from '@mika/sdk/contracts'
import { Collapsible, ConfirmButton, GlassToggle, MOTION, type PillVariant, StatusPill } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { ChevronDown, FolderOpen, KeyRound, Pencil, Server, Trash2, TriangleAlert, Wrench } from 'lucide-react'
import { useId, useState } from 'react'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { commandPreview, errorHint } from './mcp-server-form'
import { BTN } from './mcp-ui'

type T = ReturnType<typeof useI18n>['t']

type ServerLook = { variant: PillVariant; label: string; orb: string; ring: string }

function serverLook(server: McpServerSummary, t: T): ServerLook {
  if (!server.enabled) {
    return {
      variant: 'inactive',
      label: t('settings.mcp.status.disabled'),
      orb: 'bg-text-subtle',
      ring: 'border-border-subtle',
    }
  }
  if (server.status === 'connected') {
    return { variant: 'ok', label: t('settings.mcp.status.connected'), orb: 'bg-ok', ring: 'border-ok-rim' }
  }
  if (server.status === 'error') {
    return { variant: 'error', label: t('settings.mcp.status.error'), orb: 'bg-danger', ring: 'border-danger-rim' }
  }
  return {
    variant: 'pending',
    label: t('settings.mcp.status.stopped'),
    orb: 'bg-text-muted',
    ring: 'border-border-subtle',
  }
}

function StatusOrb({ look, live }: { look: ServerLook; live: boolean }) {
  return (
    <div
      aria-hidden="true"
      className={`relative flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border bg-bg-deep/60 text-text-muted ${look.ring}`}
    >
      <Server size={18} />
      <span className="absolute -right-1 -top-1 flex h-3 w-3">
        {live && (
          <span className={`absolute inline-flex h-full w-full animate-ping rounded-full opacity-60 ${look.orb}`} />
        )}
        <span className={`relative inline-flex h-3 w-3 rounded-full border-2 border-bg-surface ${look.orb}`} />
      </span>
    </div>
  )
}

function ServerError({ server, t }: { server: McpServerSummary; t: T }) {
  if (server.status !== 'error' || !server.error) return null
  return (
    <div
      role="alert"
      className="mx-4 mb-4 flex items-start gap-2.5 rounded-xl border border-danger-rim bg-danger-soft px-3.5 py-2.5"
    >
      <TriangleAlert size={15} className="mt-0.5 shrink-0 text-danger" aria-hidden="true" />
      <div className="min-w-0 space-y-1">
        <p className="break-words font-mono text-caption text-danger">{server.error}</p>
        <p className="text-caption leading-relaxed text-text-muted">
          {t(`settings.mcp.hint.${errorHint(server.error, server.command)}`)}
        </p>
      </div>
    </div>
  )
}

function UnreadableEnv({ server, t }: { server: McpServerSummary; t: T }) {
  const names = server.unreadableEnv ?? []
  if (names.length === 0) return null
  return (
    <div
      role="alert"
      className="mx-4 mb-4 flex items-start gap-2.5 rounded-xl border border-warn-rim bg-warn-soft px-3.5 py-2.5"
    >
      <KeyRound size={15} className="mt-0.5 shrink-0 text-warn" aria-hidden="true" />
      <p className="break-words text-caption leading-relaxed text-text-muted">
        {t('settings.mcp.card.unreadableEnv').replace('{{names}}', names.join(', '))}
      </p>
    </div>
  )
}

function ToolList({ server, t }: { server: McpServerSummary; t: T }) {
  const [open, setOpen] = useState(false)
  const regionId = useId()
  if (server.tools.length === 0) return null
  return (
    <div className="border-t border-border-subtle">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={regionId}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-4 py-2.5 text-caption font-semibold text-text-muted transition-colors hover:bg-surface-raise/40 hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-focus"
      >
        <Wrench size={13} aria-hidden="true" />
        {t(open ? 'settings.mcp.card.hideTools' : 'settings.mcp.card.showTools', { count: server.tools.length })}
        <ChevronDown
          size={14}
          aria-hidden="true"
          className={`ml-auto transition-transform duration-200 ${open ? 'rotate-180' : ''}`}
        />
      </button>
      <Collapsible open={open} id={regionId}>
        <ul className="grid gap-2 px-4 pb-4 sm:grid-cols-2">
          {server.tools.map((tool) => (
            <li key={tool.key ?? tool.name} className="rounded-lg border border-border-subtle bg-bg-deep/40 px-3 py-2">
              <code className="font-mono text-caption font-semibold text-accent">{tool.name}</code>
              {tool.description && (
                <p className="mt-0.5 line-clamp-2 text-caption leading-snug text-text-muted" title={tool.description}>
                  {tool.description}
                </p>
              )}
            </li>
          ))}
        </ul>
      </Collapsible>
    </div>
  )
}

type ServerCardProps = {
  server: McpServerSummary
  busy: boolean
  actionError?: string
  onToggle: (enabled: boolean) => void
  onEdit: () => void
  onRemove: () => void
}

export function McpServerCard({ server, busy, actionError, onToggle, onEdit, onRemove }: ServerCardProps) {
  const { t } = useI18n()
  const look = serverLook(server, t)
  const preview = commandPreview(server.command, server.args)
  return (
    <motion.li
      layout="position"
      {...MOTION.fadeUp}
      className={`overflow-hidden rounded-2xl border bg-surface-raise/30 transition-colors ${look.ring} ${server.enabled ? '' : 'opacity-80'}`}
    >
      <div className="flex flex-wrap items-center gap-4 p-4">
        <StatusOrb look={look} live={server.enabled && server.status === 'connected'} />
        <div className="min-w-0 flex-1 space-y-1.5">
          <div className="flex flex-wrap items-center gap-2">
            <h4 className="truncate text-sm font-bold text-text-main">{server.name}</h4>
            <StatusPill variant={look.variant} label={look.label} dot />
            {server.enabled && server.status === 'connected' && (
              <StatusPill variant="accent" label={t('settings.mcp.card.toolCount', { count: server.tools.length })} />
            )}
            {server.envKeys.length > 0 && (
              <span className="inline-flex items-center gap-1 text-caption text-text-subtle">
                <KeyRound size={12} aria-hidden="true" />
                {t('settings.mcp.card.secretCount', { count: server.envKeys.length })}
              </span>
            )}
          </div>
          <code title={preview} className="block truncate font-mono text-caption text-text-muted">
            {preview}
          </code>
          {server.cwd && (
            <p className="flex items-center gap-1 truncate text-caption text-text-subtle" title={server.cwd}>
              <FolderOpen size={12} aria-hidden="true" />
              {server.cwd}
            </p>
          )}
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <div className="w-11">
            <GlassToggle
              label={t('settings.mcp.card.toggle').replace('{{name}}', server.name)}
              hideLabel
              value={server.enabled}
              disabled={busy}
              onChange={onToggle}
            />
          </div>
          <button
            type="button"
            className={BTN.icon}
            onClick={onEdit}
            disabled={busy}
            aria-label={t('settings.mcp.card.edit').replace('{{name}}', server.name)}
            title={t('settings.mcp.card.edit').replace('{{name}}', server.name)}
          >
            <Pencil size={14} aria-hidden="true" />
          </button>
          <ConfirmButton
            icon={Trash2}
            ariaLabel={t('settings.mcp.card.remove').replace('{{name}}', server.name)}
            confirmLabel={t('settings.mcp.card.removeConfirm')}
            onConfirm={onRemove}
            disabled={busy}
          />
        </div>
      </div>
      {actionError && (
        <p role="alert" className="mx-4 mb-3 text-caption text-danger">
          {actionError}
        </p>
      )}
      <UnreadableEnv server={server} t={t} />
      <ServerError server={server} t={t} />
      <ToolList server={server} t={t} />
    </motion.li>
  )
}
