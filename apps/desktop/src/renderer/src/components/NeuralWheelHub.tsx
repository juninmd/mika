import { useI18n } from '../hooks/useI18n'
import type { WheelItem } from './use-neural-wheel-items'

export function NeuralWheelHub({
  isGamesView,
  accentColor,
  activeItem,
  onBack,
}: {
  isGamesView: boolean
  accentColor: string
  activeItem: WheelItem | null
  onBack: () => void
}) {
  const { t } = useI18n()
  return isGamesView ? (
    <button
      type="button"
      onClick={onBack}
      data-testid="neural-wheel-back"
      aria-label={t('shell.wheel.hub.backAriaLabel')}
      className="absolute flex h-24 w-24 flex-col items-center justify-center gap-1 rounded-full border-4 bg-bg-surface transition-transform hover:scale-105 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-strong"
      style={{ borderColor: accentColor, boxShadow: `0 0 30px ${accentColor}` }}
    >
      <span className="text-micro font-black uppercase tracking-[0.2em]" style={{ color: accentColor }}>
        {t('shell.wheel.hub.back')}
      </span>
      <div className="h-1 w-8 rounded-full" style={{ backgroundColor: accentColor }} />
    </button>
  ) : (
    <div
      aria-hidden="true"
      className="absolute w-24 h-24 rounded-full bg-bg-surface border-4"
      style={{ borderColor: accentColor, boxShadow: `0 0 30px ${accentColor}` }}
    >
      <div className="flex flex-col items-center justify-center h-full gap-1 px-2 text-center">
        <span
          className={`text-micro font-black uppercase tracking-[0.2em] ${activeItem ? '' : 'animate-pulse'}`}
          style={{ color: accentColor }}
        >
          {t('shell.wheel.hub.title')}
        </span>
        {activeItem ? (
          <div className="flex flex-col items-center">
            <span className="text-micro font-bold uppercase leading-tight tracking-tight text-text-main">
              {activeItem.label}
            </span>
            {activeItem.subLabel && (
              <span className="text-micro font-mono tracking-tight text-text-muted">{activeItem.subLabel}</span>
            )}
          </div>
        ) : (
          <div className="w-8 h-1 rounded-full" style={{ backgroundColor: accentColor }} />
        )}
      </div>
    </div>
  )
}
