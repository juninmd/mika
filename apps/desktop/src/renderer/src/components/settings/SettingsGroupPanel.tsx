import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { memo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { rendererLog } from '../../lib/renderer-logger'
import { resolveFeatureComponent } from './feature-components'

function logMissingPanel(opts: {
  tabId?: string
  pluginId?: string
  group: { title: string; groupPrefix?: string; component?: string }
  CustomComponent: unknown
}): void {
  if (opts.group.component && !opts.CustomComponent) {
    rendererLog.error('[Settings] Manual settings panel missing for group', {
      tabId: opts.tabId,
      pluginId: opts.pluginId,
      groupTitle: opts.group.title,
      groupPrefix: opts.group.groupPrefix,
      component: opts.group.component,
    })
  }
}

type SettingsGroupPanelProps = {
  config: Record<string, unknown>
  group: FeatureSettingsDeclaration['groups'][number]
  onSave: (path: string, value: unknown) => Promise<void>
  tabId?: string
  pluginId?: string
}

export const SettingsGroupPanel = memo(function SettingsGroupPanel({
  config,
  group,
  onSave,
  tabId,
  pluginId,
}: SettingsGroupPanelProps) {
  const CustomComponent = resolveFeatureComponent(group.component)
  logMissingPanel({ tabId, pluginId, group, CustomComponent })

  return (
    <section aria-label={group.title}>
      {CustomComponent ? (
        <CustomComponent config={config} onSave={onSave} tabId={tabId ?? ''} pluginId={pluginId} />
      ) : (
        <MissingPanelNotice group={group} tabId={tabId} pluginId={pluginId} />
      )}
    </section>
  )
})

function MissingPanelNotice({
  group,
  tabId,
  pluginId,
}: {
  group: SettingsGroupPanelProps['group']
  tabId?: string
  pluginId?: string
}) {
  const { t } = useI18n()
  return (
    <div role="alert" aria-live="assertive" className="rounded-xl border border-danger-rim bg-danger-soft p-4">
      <p className="text-caption font-black uppercase tracking-widest text-danger">
        {t('settings.groupPanel.missingTitle')}
      </p>
      <p className="mt-2 text-[13px] text-text-main">
        {t('settings.groupPanel.missingGroupPrefix')} <span className="font-bold text-text-main">{group.title}</span>{' '}
        {t('settings.groupPanel.missingDeclares')}{' '}
        <span className="rounded bg-danger-soft/60 px-1 py-0.5 font-mono font-semibold text-danger">
          {group.component ?? t('settings.groupPanel.noComponent')}
        </span>
        , {t('settings.groupPanel.missingNotRegistered')}{' '}
        <span className="font-mono text-text-main">feature-components.tsx</span>.
      </p>
      <p className="mt-1 text-caption text-text-muted">
        {t('settings.groupPanel.missingNothingGenerated', { count: group.fields.length })}{' '}
        <span className="font-mono">{pluginId ?? tabId ?? t('settings.groupPanel.unknown')}</span>{' '}
        {t('settings.groupPanel.missingNoUiUntilPanel')}
      </p>
    </div>
  )
}
