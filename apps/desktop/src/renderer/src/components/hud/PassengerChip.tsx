import { Ghost } from 'lucide-react'
import { memo } from 'react'
import { saveConfigFlag, useConfigFlag } from '../../hooks/useConfigFlag'
import { useI18n } from '../../hooks/useI18n'

export const PASSENGER_PATH = 'privacy.passengerMode'

/** Permanent badge while passenger mode (`privacy::passenger`) is on: nothing of this session is saved. Click turns it off. */
export const PassengerChip = memo(function PassengerChip() {
  const { t } = useI18n()
  const on = useConfigFlag(PASSENGER_PATH, false)

  if (!on) return null
  return (
    <button
      type="button"
      onClick={() => saveConfigFlag(PASSENGER_PATH, false)}
      title={t('shell.passenger.chip.hint')}
      className="pointer-events-auto flex items-center gap-2 rounded-full border border-accent-rim/40 bg-bg-surface/90 px-3 py-1.5 text-caption font-semibold text-text-main backdrop-blur-md shadow-xs transition active:scale-95 hover:border-accent-rim hover:bg-surface-raise-strong focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
    >
      <Ghost aria-hidden="true" size={14} />
      <span>{t('shell.passenger.chip.label')}</span>
    </button>
  )
})
