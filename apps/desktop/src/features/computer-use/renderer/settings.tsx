import { BentoGrid, GlassCard, GlassSection, GlassToggle, SectionDivider, StatusPill } from '@mika/sdk-ui/settings'
import {
  AppWindow,
  BadgeCheck,
  Keyboard,
  type LucideIcon,
  Mouse,
  MousePointer2,
  MousePointerClick,
  OctagonX,
  Power,
  ScanSearch,
  ShieldBan,
  TextCursorInput,
  Volume2,
} from 'lucide-react'
import type { FeatureComponentProps } from '../../../renderer/src/components/settings/feature-components'
import { configBool } from '../../../renderer/src/components/settings/settings-utils'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import { ComputerUseOsSupport } from './os-support'

function InfoTile({ icon: Icon, title, text }: { icon: LucideIcon; title: string; text: string }) {
  return (
    <div className="flex min-w-0 items-start gap-3 rounded-xl border border-border-subtle bg-surface-raise p-3">
      <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-accent-rim bg-accent-soft text-accent">
        <Icon size={15} aria-hidden="true" />
      </span>
      <div className="min-w-0">
        <p className="text-micro font-black uppercase tracking-[0.12em] text-text-main">{title}</p>
        <p className="mt-1 text-caption leading-snug text-text-muted">{text}</p>
      </div>
    </div>
  )
}

/** Single kill switch for `computer` (approval per call) and `pc_control` (volume/windows, no
 *  dialog); Ctrl+Alt+End turns it off from anywhere (`emergency_stop.rs`). */
export function ComputerUsePanel({ config, onSave }: FeatureComponentProps) {
  const { t } = useI18n()
  const saveBool = onSave as (path: string, value: boolean) => Promise<void>
  const enabled = configBool(config, 'computer-use.enabled', false)

  return (
    <GlassSection title={t('settings.computerUse.tab.label')} icon={MousePointerClick}>
      <BentoGrid cols="auto">
        <GlassCard span="full" hover={false}>
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="min-w-0 flex-1">
              <GlassToggle
                label={t('settings.computerUse.field.enabled.label')}
                description={t('settings.computerUse.field.enabled.description')}
                configPath="computer-use.enabled"
                value={enabled}
                onSave={(_p, v) => saveBool('computer-use.enabled', v as boolean)}
              />
            </div>
            <StatusPill
              variant={enabled ? 'ok' : 'inactive'}
              label={enabled ? t('settings.computerUse.panel.statusOn') : t('settings.computerUse.panel.statusOff')}
              dot
            />
          </div>
        </GlassCard>

        <GlassCard span="full" hover={false}>
          <h3 className="text-sm font-black uppercase tracking-[0.12em] text-text-main">
            {t('settings.computerUse.panel.safeguardsTitle')}
          </h3>
          <p className="mt-1 text-xs leading-relaxed text-text-muted">
            {t('settings.computerUse.panel.safeguardsDescription')}
          </p>
          <div className="mt-4 grid gap-2 md:grid-cols-3 xl:grid-cols-5">
            <InfoTile
              icon={Power}
              title={t('settings.computerUse.panel.guardSwitchTitle')}
              text={t('settings.computerUse.panel.guardSwitchText')}
            />
            <InfoTile
              icon={BadgeCheck}
              title={t('settings.computerUse.panel.guardApprovalTitle')}
              text={t('settings.computerUse.panel.guardApprovalText')}
            />
            <InfoTile
              icon={ShieldBan}
              title={t('settings.computerUse.panel.guardWindowTitle')}
              text={t('settings.computerUse.panel.guardWindowText')}
            />
            <InfoTile
              icon={OctagonX}
              title={t('settings.computerUse.panel.guardStopTitle')}
              text={t('settings.computerUse.panel.guardStopText')}
            />
            <InfoTile
              icon={MousePointer2}
              title={t('settings.computerUse.panel.guardCursorTitle')}
              text={t('settings.computerUse.panel.guardCursorText')}
            />
          </div>

          <SectionDivider label={t('settings.computerUse.panel.actionsTitle')} />
          <div className="grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
            <InfoTile
              icon={ScanSearch}
              title={t('settings.computerUse.panel.actionFind')}
              text={t('settings.computerUse.panel.actionFindText')}
            />
            <InfoTile
              icon={MousePointerClick}
              title={t('settings.computerUse.panel.actionClick')}
              text={t('settings.computerUse.panel.actionClickText')}
            />
            <InfoTile
              icon={TextCursorInput}
              title={t('settings.computerUse.panel.actionType')}
              text={t('settings.computerUse.panel.actionTypeText')}
            />
            <InfoTile
              icon={Keyboard}
              title={t('settings.computerUse.panel.actionKey')}
              text={t('settings.computerUse.panel.actionKeyText')}
            />
            <InfoTile
              icon={Mouse}
              title={t('settings.computerUse.panel.actionMouse')}
              text={t('settings.computerUse.panel.actionMouseText')}
            />
            <InfoTile
              icon={Volume2}
              title={t('settings.computerUse.panel.actionVolume')}
              text={t('settings.computerUse.panel.actionVolumeText')}
            />
            <InfoTile
              icon={AppWindow}
              title={t('settings.computerUse.panel.actionWindow')}
              text={t('settings.computerUse.panel.actionWindowText')}
            />
          </div>
          <p className="mt-3 text-caption leading-snug text-text-muted">
            {t('settings.computerUse.panel.actionsDescription')}
          </p>
        </GlassCard>

        <ComputerUseOsSupport />
      </BentoGrid>
    </GlassSection>
  )
}
