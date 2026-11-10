import { useI18n } from '../../hooks/useI18n'

export function BootOverlay() {
  const { t } = useI18n()
  return (
    <div
      role="status"
      aria-live="polite"
      className="avatar-enter absolute left-1/2 top-3 -translate-x-1/2 overflow-hidden whitespace-nowrap rounded-full border border-accent-rim bg-bg-surface/90 text-accent px-4 py-1.5 text-caption font-semibold tracking-[0.18em] shadow-2xl backdrop-blur"
      style={{
        zIndex: 'var(--layer-toasts)',
      }}
    >
      <span
        aria-hidden="true"
        className="sheen-sweep pointer-events-none absolute inset-y-0 left-0 w-1/3 bg-gradient-to-r from-transparent via-accent-soft to-transparent"
      />
      <span className="relative">{t('shell.bootOverlay.connecting')}</span>
    </div>
  )
}
