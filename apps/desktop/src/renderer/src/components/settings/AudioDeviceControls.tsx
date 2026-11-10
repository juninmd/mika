import { GlassSelect, StatusPill } from '@mika/sdk-ui/settings'
import type { Mic } from 'lucide-react'
import { useI18n } from '../../hooks/useI18n'

export function MicLevelBars({ level }: { level: number }) {
  const { t } = useI18n()
  const bars = 20
  const roundedLevel = Math.round(level)
  return (
    <div
      role="progressbar"
      aria-label={t('settings.audioDevices.micLevelAriaLabel')}
      aria-valuenow={roundedLevel}
      aria-valuemin={0}
      aria-valuemax={100}
      className="flex items-end gap-0.5 h-12"
    >
      {Array.from({ length: bars }, (_, i) => {
        const threshold = (i / bars) * 100
        const active = level > threshold
        const tone = level > 70 ? 'var(--danger)' : level > 40 ? 'var(--warn)' : 'var(--ok)'
        return (
          <div
            key={`lvl-${Math.round(threshold)}`}
            className="w-1.5 rounded-sm transition duration-75"
            style={{
              height: `${30 + (i / bars) * 70}%`,
              background: active ? tone : 'var(--border-subtle)',
              boxShadow: active ? `0 0 6px color-mix(in srgb, ${tone} 40%, transparent)` : 'none',
            }}
          />
        )
      })}
    </div>
  )
}

// Chromium lists the OS default endpoint as deviceId "default"; our own entry already covers it.
export function deviceOptions(devices: { deviceId: string; label: string }[], defaultLabel: string) {
  return [
    { value: 'default', label: defaultLabel },
    ...devices.filter((d) => d.deviceId !== 'default').map((d) => ({ value: d.deviceId, label: d.label })),
  ]
}

export function DeviceSelect({
  label,
  icon: Icon,
  devices,
  value,
  onChange,
}: {
  label: string
  icon: typeof Mic
  devices: { deviceId: string; label: string }[]
  value: string
  onChange: (id: string) => void
}) {
  const { t } = useI18n()
  const options = deviceOptions(devices, t('settings.audioDevices.systemDefault'))
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2.5">
        <span
          aria-hidden="true"
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent"
        >
          <Icon size={16} />
        </span>
        <p className="min-w-0 flex-1 text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{label}</p>
        <StatusPill
          variant="neutral"
          label={t('settings.audioDevices.deviceCount').replace('{{count}}', String(options.length - 1))}
        />
      </div>
      <GlassSelect label={label} hideLabel value={value} onChange={onChange} variant="dropdown" options={options} />
    </div>
  )
}
