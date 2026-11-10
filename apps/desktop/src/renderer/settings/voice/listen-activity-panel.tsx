import { GlassCard } from '@mika/sdk-ui/settings'
import { Mic, MicOff } from 'lucide-react'
import { useEffect, useState } from 'react'
import { asrProviderLabels, labelOf } from '../../src/components/settings/settings-utils'
import { useI18n } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'

function VoiceBars({ listening }: { listening: boolean }) {
  const heights = [3, 6, 9, 5, 8, 4, 7]
  return (
    <div className="flex items-end gap-0.5" style={{ height: 16 }}>
      {heights.map((h, i) => (
        <div
          key={h}
          className="w-1 rounded-sm"
          style={{
            height: listening ? h : 3,
            background: 'var(--accent)',
            opacity: listening ? 0.85 : 0.2,
            animation: listening ? `mika-voicebar 900ms ease-in-out ${i * 90}ms infinite alternate` : undefined,
          }}
        />
      ))}
      {listening && (
        <style>{`@keyframes mika-voicebar { from { transform: scaleY(0.4) } to { transform: scaleY(1.15) } }`}</style>
      )}
    </div>
  )
}

type Props = {
  asrProvider: string
}

export function ListenActivityPanel({ asrProvider }: Props) {
  const { t } = useI18n()
  const [micOn, setMicOn] = useState(false)

  useEffect(() => {
    return getElectronApi().runtime.onMicState?.(({ enabled }) => setMicOn(enabled))
  }, [])

  const accentColor = 'var(--accent-text)'

  return (
    <div className="space-y-4">
      <GlassCard>
        <div className="flex items-center gap-3">
          <div
            className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-full border ${
              micOn ? 'border-accent-rim bg-accent-soft' : 'border-border-subtle bg-bg-deep'
            }`}
          >
            {micOn ? (
              <Mic size={16} style={{ color: accentColor }} />
            ) : (
              <MicOff size={16} className="text-text-subtle" />
            )}
          </div>

          <div className="flex flex-1 flex-col gap-1">
            <div className="flex flex-wrap items-center justify-between gap-x-3 gap-y-1">
              <span className="text-caption font-semibold" style={{ color: micOn ? accentColor : undefined }}>
                {micOn ? t('settings.voiceStt.panel.micListening') : t('settings.voiceStt.panel.micOff')}
              </span>
              <span className="text-caption text-text-muted">{labelOf(asrProviderLabels(t), asrProvider)}</span>
            </div>
            <VoiceBars listening={micOn} />
          </div>
        </div>
      </GlassCard>
    </div>
  )
}
