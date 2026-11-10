import type { McpServerSummary } from '@mika/sdk/contracts'
import { BentoGrid, EmptyState, GlassCard, GlassSection, GlassSlider, StatusPill } from '@mika/sdk-ui/settings'
import { AnimatePresence } from 'framer-motion'
import { FileX, Plus, Server, SlidersHorizontal, Sparkles } from 'lucide-react'
import { useState } from 'react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool, configNum } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { ConfigPathRow, McpBanners, McpEmptyState, McpLoadFailed, McpSkeleton } from './mcp-panel-parts'
import { McpServerCard } from './mcp-server-card'
import { type EditorTarget, McpServerEditor } from './mcp-server-editor'
import { emptyForm, formFromServer, formFromTemplate } from './mcp-server-form'
import { McpTemplatesGallery } from './mcp-templates-gallery'
import { BTN } from './mcp-ui'
import { useMcpServers } from './use-mcp-servers'

type T = ReturnType<typeof useI18n>['t']

function ServersSummary({ servers, t }: { servers: readonly McpServerSummary[]; t: T }) {
  const live = servers.filter((s) => s.enabled && s.status === 'connected').length
  const failing = servers.filter((s) => s.enabled && s.status === 'error').length
  const tools = servers.reduce((n, s) => n + s.tools.length, 0)
  return (
    <div className="flex flex-wrap items-center gap-2">
      <StatusPill
        variant={live > 0 ? 'ok' : 'inactive'}
        dot
        label={t('settings.mcp.summary.connected').replace('{{count}}', String(live))}
      />
      {failing > 0 && (
        <StatusPill
          variant="error"
          dot
          label={t('settings.mcp.summary.failing').replace('{{count}}', String(failing))}
        />
      )}
      <StatusPill variant="accent" label={t('settings.mcp.summary.tools', { count: tools })} />
    </div>
  )
}

type ServersBodyProps = {
  mcp: ReturnType<typeof useMcpServers>
  onAdd: () => void
  onEdit: (server: McpServerSummary) => void
}

function ServersBody({ mcp, onAdd, onEdit }: ServersBodyProps) {
  const { t } = useI18n()
  if (mcp.state.status === 'loading') return <McpSkeleton />
  if (mcp.state.status === 'failed') return <McpLoadFailed error={mcp.state.error} onRetry={() => void mcp.reload()} />
  const { servers, error } = mcp.state.data
  if (error !== null) {
    return (
      <EmptyState
        icon={FileX}
        title={t('settings.mcp.servers.lockedTitle')}
        hint={t('settings.mcp.servers.lockedHint')}
      />
    )
  }
  if (servers.length === 0) return <McpEmptyState onAdd={onAdd} disabled={false} />
  return (
    <div className="space-y-3">
      <ServersSummary servers={servers} t={t} />
      <ul className="space-y-3">
        <AnimatePresence initial={false}>
          {servers.map((server) => (
            <McpServerCard
              key={server.name}
              server={server}
              busy={Boolean(mcp.busy[server.name])}
              actionError={mcp.actionError?.name === server.name ? mcp.actionError.message : undefined}
              onToggle={(enabled) => void mcp.toggle(server, enabled)}
              onEdit={() => onEdit(server)}
              onRemove={() => void mcp.remove(server.name)}
            />
          ))}
        </AnimatePresence>
      </ul>
    </div>
  )
}

export function McpPanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const timeoutSeconds = configNum(config, 'mcp.timeoutSeconds', 20)
  const mcp = useMcpServers(configBool(config, 'mcp.enabled'))
  const [editor, setEditor] = useState<EditorTarget | null>(null)

  const data = mcp.state.status === 'ready' ? mcp.state.data : null
  const servers = data?.servers ?? []
  const names = servers.map((s) => s.name)
  // mcp.json ilegível: o backend recusa gravar até o usuário corrigir o arquivo.
  const locked = !data || data.error !== null
  const openNew = () => setEditor({ form: emptyForm() })
  const panelError = mcp.actionError && mcp.actionError.name === '' ? mcp.actionError.message : null

  return (
    <div className="space-y-6">
      {data && <McpBanners data={data} onOpenDir={() => void mcp.openConfigDir()} />}
      {panelError && (
        <p
          role="alert"
          className="rounded-xl border border-danger-rim bg-danger-soft px-4 py-2.5 text-caption text-danger"
        >
          {panelError}
        </p>
      )}

      <GlassSection
        title={t('settings.mcp.servers.title')}
        icon={Server}
        badge={data ? String(servers.length) : undefined}
        description={t('settings.mcp.servers.description')}
        actions={
          servers.length > 0 ? (
            <button type="button" className={BTN.primary} onClick={openNew} disabled={locked}>
              <Plus size={14} aria-hidden="true" />
              {t('settings.mcp.servers.add')}
            </button>
          ) : undefined
        }
      >
        <ServersBody
          mcp={mcp}
          onAdd={openNew}
          onEdit={(server) => setEditor({ form: formFromServer(server), originalName: server.name })}
        />
      </GlassSection>

      <GlassSection
        title={t('settings.mcp.gallery.title')}
        icon={Sparkles}
        description={t('settings.mcp.gallery.description')}
      >
        <McpTemplatesGallery
          servers={servers}
          disabled={locked}
          onPick={(template) => setEditor({ form: formFromTemplate(template, names), template })}
        />
      </GlassSection>

      <GlassSection
        title={t('settings.mcp.advanced.title')}
        icon={SlidersHorizontal}
        description={t('settings.mcp.advanced.description')}
      >
        <BentoGrid cols={2}>
          <GlassCard hover={false}>
            {data ? (
              <ConfigPathRow path={data.configPath} onOpenDir={() => void mcp.openConfigDir()} />
            ) : (
              <div className="h-16 animate-pulse rounded-lg bg-surface-raise/60" />
            )}
          </GlassCard>
          <GlassCard hover={false}>
            <GlassSlider
              label={t('settings.mcp.panel.timeoutLabel')}
              description={t('settings.mcp.panel.timeoutDescription')}
              configPath="mcp.timeoutSeconds"
              value={timeoutSeconds}
              min={2}
              max={120}
              step={1}
              showValue={(v) => `${v}s`}
              onSave={onSave}
            />
          </GlassCard>
        </BentoGrid>
      </GlassSection>

      <AnimatePresence>
        {editor && <McpServerEditor target={editor} takenNames={names} onClose={() => setEditor(null)} />}
      </AnimatePresence>
    </div>
  )
}
