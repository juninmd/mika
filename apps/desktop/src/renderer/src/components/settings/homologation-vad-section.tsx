import { Mic, Play, SlidersHorizontal, Square } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'
import { GlassCard, SectionLabel } from './ds'
import { useHomologationVad } from './use-homologation-vad'

/**
 * Live mic → VAD → STT round trip. Sensitivity is NOT tuned here: an unsaved slider looked like a
 * setting and changed nothing outside this card, so calibration lives in Escuta (STT).
 */
export function HomologationVadSection({ onOpenStt }: { onOpenStt: () => void }) {
  const { t } = useI18n()
  const {
    vadActive,
    audioLevel,
    vadSpeaking,
    vadHistory,
    vadThreshold,
    vadStatus,
    sttText,
    sttLoading,
    startVadTest,
    stopVadTest,
  } = useHomologationVad()

  return (
    <GlassCard className="flex flex-col h-full">
      <SectionLabel icon={Mic} className="mb-4">
        {t('settings.homologation.vad.title')}
      </SectionLabel>

      <div className="space-y-4 flex-1 flex flex-col justify-between">
        <div className="space-y-3">
          <p className="text-xs text-text-muted">{t('settings.homologation.vad.description')}</p>

          <button
            type="button"
            onClick={onOpenStt}
            className="inline-flex items-center gap-1.5 rounded-lg border border-border-subtle bg-surface-raise px-3 py-1.5 text-caption font-semibold text-text-main transition hover:border-accent-rim hover:bg-accent-soft hover:text-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            <SlidersHorizontal size={12} aria-hidden="true" />
            {t('settings.diagnostics.calibrateMic')}
          </button>

          <div className="pt-2">
            <div className="flex items-center justify-between mb-1.5">
              <span className="text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
                {t('settings.homologation.vad.volumeLevel')}
              </span>
              {vadSpeaking && (
                <span className="text-caption px-1.5 py-0.5 rounded bg-ok-soft text-ok border border-ok-rim animate-pulse font-bold">
                  {t('settings.homologation.vad.speechDetected')}
                </span>
              )}
            </div>
            <div
              role="progressbar"
              aria-valuenow={Math.round(audioLevel * 100)}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-label={t('settings.homologation.vad.volumeLevelAria')}
              className="w-full h-3 bg-surface-raise/40 rounded-full overflow-hidden border border-border-subtle p-[2px]"
            >
              <div
                className="h-full w-full origin-left rounded-full transition-transform duration-75"
                style={{
                  transform: `scaleX(${Math.min(1, Math.max(0, audioLevel))})`,
                  background:
                    audioLevel > vadThreshold
                      ? 'linear-gradient(90deg, var(--ok), var(--ok-soft))'
                      : 'linear-gradient(90deg, var(--info), var(--primary))',
                }}
              />
            </div>
          </div>
        </div>

        <div className="mt-4 space-y-3">
          {vadActive ? (
            <button
              type="button"
              onClick={stopVadTest}
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-danger-rim bg-danger-soft py-2 text-xs font-bold text-danger transition-colors hover:bg-danger/30 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <Square size={12} aria-hidden="true" />
              {t('settings.homologation.vad.stopButton')}
            </button>
          ) : (
            <button
              type="button"
              onClick={startVadTest}
              className="flex w-full items-center justify-center gap-2 rounded-xl border border-ok-rim bg-ok-soft py-2 text-xs font-bold text-ok transition-colors hover:bg-ok/25 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              <Play size={12} className="fill-current" aria-hidden="true" />
              {t('settings.homologation.vad.startButton')}
            </button>
          )}

          <div className="p-3 rounded-lg bg-surface-raise/40 border border-border-subtle text-xs space-y-2">
            <div className="flex items-center justify-between gap-2">
              <span className="min-w-0 truncate font-mono text-caption text-text-muted" title={vadStatus}>
                {vadStatus}
              </span>
              {sttLoading && (
                <span className="text-caption text-ok animate-pulse font-bold">
                  {t('settings.homologation.vad.transcribing')}
                </span>
              )}
            </div>

            {sttText && (
              <div className="pt-1 border-t border-border-subtle">
                <span className="mb-1 block text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
                  {t('settings.homologation.vad.lastTranscription')}
                </span>
                <p className="text-text-main italic leading-relaxed">"{sttText}"</p>
              </div>
            )}

            {vadHistory.length > 0 && (
              <div className="pt-2 border-t border-border-subtle space-y-1">
                <span className="block text-micro font-semibold uppercase tracking-[0.12em] text-text-muted">
                  {t('settings.homologation.vad.testHistory')}
                </span>
                {vadHistory.map((h) => (
                  <p key={h.id} className="text-caption text-text-muted truncate">
                    • {h.text}
                  </p>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </GlassCard>
  )
}
