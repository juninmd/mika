import type { PillVariant } from '@mika/sdk-ui/settings'
import { AudioLines, Mic, MicOff, Volume2 } from 'lucide-react'
import { useState } from 'react'
import { useAudioDevices } from '../../hooks/useAudioDevices'
import { type Translate, useI18n } from '../../hooks/useI18n'
import { DeviceSelect, MicLevelBars } from './AudioDeviceControls'
import { GlassCard, GlassSection, PageHero, SectionHeader, StatusPill } from './ds'

type Props = { onSave: (path: string, value: unknown) => Promise<void> }

function playTestTone(sinkId?: string): void {
  try {
    const ctx = new AudioContext()
    if (sinkId && sinkId !== 'default' && 'setSinkId' in ctx) {
      void (ctx as unknown as { setSinkId: (id: string) => Promise<void> }).setSinkId(sinkId).catch(() => {})
    }
    const osc = ctx.createOscillator()
    const gain = ctx.createGain()
    const now = ctx.currentTime
    osc.type = 'sine'
    osc.frequency.setValueAtTime(520, now)
    osc.frequency.exponentialRampToValueAtTime(880, now + 0.22)
    gain.gain.setValueAtTime(0.001, now)
    gain.gain.exponentialRampToValueAtTime(0.12, now + 0.04)
    gain.gain.exponentialRampToValueAtTime(0.001, now + 0.3)
    osc.connect(gain).connect(ctx.destination)
    osc.start(now)
    osc.stop(now + 0.35)
    setTimeout(() => {
      void ctx.close().catch(() => {})
    }, 450)
  } catch {
    // AudioContext unavailable in test or restricted environment
  }
}

type MicState = ReturnType<typeof useAudioDevices>['permissionState']

const PERMISSION_PILL: Record<MicState, { variant: PillVariant; key: Parameters<Translate>[0] }> = {
  granted: { variant: 'ok', key: 'settings.audioDevices.permission.granted' },
  unknown: { variant: 'pending', key: 'settings.audioDevices.permission.unknown' },
  denied: { variant: 'error', key: 'settings.audioDevices.permission.denied' },
  'not-found': { variant: 'warn', key: 'settings.audioDevices.permission.notFound' },
  'device-error': { variant: 'error', key: 'settings.audioDevices.permission.error' },
}

export function AudioDevicesPanel({ onSave }: Props) {
  const { t } = useI18n()
  const audio = useAudioDevices(onSave)
  const [isPlayingTest, setIsPlayingTest] = useState(false)

  const handleTestTone = () => {
    setIsPlayingTest(true)
    playTestTone(audio.selectedOutputId)
    setTimeout(() => setIsPlayingTest(false), 450)
  }

  const permission = PERMISSION_PILL[audio.permissionState]
  const inputCount = audio.inputs.filter((d) => d.deviceId !== 'default').length
  const outputCount = audio.outputs.filter((d) => d.deviceId !== 'default').length

  return (
    <div className="space-y-6">
      <PageHero
        icon={AudioLines}
        title={t('settings.audioDevices.title')}
        description={t('shell.settingsPanel.audioDevicesDescription')}
        note={t('settings.audioDevices.heroNote')}
        pills={
          <>
            <StatusPill variant="neutral" label={t('settings.audioDevices.badge')} />
            <StatusPill variant={permission.variant} label={t(permission.key)} dot />
            <StatusPill
              variant="neutral"
              label={`${t('settings.audioDevices.inputsShort')}: ${inputCount} · ${t('settings.audioDevices.outputsShort')}: ${outputCount}`}
            />
          </>
        }
      />
      <GlassSection
        title={t('settings.audioDevices.routingTitle')}
        icon={Volume2}
        description={t('settings.audioDevices.routingDescription')}
      >
        <div className="space-y-5">
          {audio.permissionState === 'denied' && (
            <div role="alert" className="rounded-xl border border-danger-rim bg-danger-soft px-4 py-3">
              <p className="text-xs text-danger">{t('settings.audioDevices.permissionDenied')}</p>
            </div>
          )}

          {audio.permissionState === 'not-found' && (
            <div role="alert" className="rounded-xl border border-danger-rim bg-danger-soft px-4 py-3">
              <p className="text-xs text-danger">{t('settings.audioDevices.notFound')}</p>
            </div>
          )}

          {audio.permissionState === 'device-error' && (
            <div role="alert" className="rounded-xl border border-danger-rim bg-danger-soft px-4 py-3">
              <p className="text-xs text-danger">{t('settings.audioDevices.deviceError')}</p>
            </div>
          )}

          {audio.permissionState === 'unknown' && audio.inputs.length === 0 && (
            <button
              type="button"
              onClick={() => void audio.requestPermission()}
              className="w-full rounded-xl border border-accent-rim bg-accent-soft py-3 text-sm font-semibold text-accent-text transition-colors active:scale-[0.98] hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('settings.audioDevices.allowMicAccess')}
            </button>
          )}

          <div className="grid gap-4 sm:grid-cols-2">
            <GlassCard>
              <DeviceSelect
                label={t('settings.audioDevices.inputLabel')}
                icon={Mic}
                devices={audio.inputs}
                value={audio.selectedInputId}
                onChange={(id) => void audio.setSelectedInputId(id)}
              />
            </GlassCard>
            <GlassCard>
              <DeviceSelect
                label={t('settings.audioDevices.outputLabel')}
                icon={Volume2}
                devices={audio.outputs}
                value={audio.selectedOutputId}
                onChange={(id) => void audio.setSelectedOutputId(id)}
              />
              <div className="mt-3 flex justify-end">
                <button
                  type="button"
                  onClick={handleTestTone}
                  disabled={isPlayingTest}
                  aria-label={t('settings.audioDevices.testOutputAriaLabel')}
                  className={`flex items-center gap-1.5 rounded-lg border px-2.5 py-1 text-caption font-semibold transition active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                    isPlayingTest
                      ? 'border-info-rim bg-info-soft text-info animate-pulse'
                      : 'border-border-subtle bg-surface-raise text-text-muted hover:border-border-strong hover:bg-surface-raise-strong hover:text-text-main'
                  }`}
                >
                  <Volume2 size={12} aria-hidden="true" />
                  <span>
                    {isPlayingTest ? t('settings.audioDevices.playingTest') : t('settings.audioDevices.testSound')}
                  </span>
                </button>
              </div>
            </GlassCard>
          </div>

          <GlassCard accent>
            <SectionHeader
              title={t('settings.audioDevices.micTestTitle')}
              description={t('settings.audioDevices.micTestDescription')}
              actions={
                <button
                  type="button"
                  onClick={() => void audio.startMonitoring()}
                  aria-pressed={audio.isMonitoring}
                  className={`flex items-center gap-2 rounded-lg border px-3 py-1.5 text-caption font-bold transition active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
                    audio.isMonitoring
                      ? 'border-danger-rim bg-danger-soft text-danger hover:bg-danger hover:text-white'
                      : 'border-border-subtle bg-surface-raise text-text-main hover:border-border-strong'
                  }`}
                >
                  {audio.isMonitoring ? <MicOff size={13} aria-hidden="true" /> : <Mic size={13} aria-hidden="true" />}
                  {audio.isMonitoring ? t('settings.audioDevices.stop') : t('settings.audioDevices.test')}
                </button>
              }
            />
            <MicLevelBars level={audio.micLevel} />
            {audio.isMonitoring && (
              <p role="status" aria-live="polite" className="text-center text-caption text-text-muted tabular-nums">
                {t('settings.audioDevices.levelLabel')}{' '}
                <span className="text-ok font-mono font-bold">{Math.round(audio.micLevel)}%</span>
              </p>
            )}
          </GlassCard>
        </div>
      </GlassSection>
    </div>
  )
}
