import { CalendarCheck, Ghost, History, Lightbulb } from 'lucide-react'
import { saveConfigFlag, useConfigFlag } from '../../hooks/useConfigFlag'
import { useI18n } from '../../hooks/useI18n'
import { PASSENGER_PATH } from '../hud/PassengerChip'
import { RAIL_BUTTON, RAIL_ON } from './dock-ui'

/**
 * One-tap asks from the rail, all over the features that already exist: "where did we leave off?"
 * (the last session recap, `gamer::session_memory::resume_recap`), "what did we do today?" (the day
 * summary, `gamer::daily_summary`), a spoiler-free hint that climbs
 * a step each time it is asked again (`gamer::game_knowledge_tool::HelpLadder`), and passenger mode
 * (`privacy::passenger`). The asks go out as the user's own message: she answers, never a canned line.
 */
export function DockQuickActions({ onAsk }: { onAsk: (text: string) => void }) {
  const { t } = useI18n()
  const passenger = useConfigFlag(PASSENGER_PATH, false)
  return (
    <>
      <button
        type="button"
        onClick={() => onAsk(t('shell.edgeDock.quick.recapPrompt'))}
        title={t('shell.edgeDock.quick.recap')}
        className={`${RAIL_BUTTON} border-transparent`}
      >
        <History size={18} />
      </button>
      <button
        type="button"
        onClick={() => onAsk(t('shell.edgeDock.quick.todayPrompt'))}
        title={t('shell.edgeDock.quick.today')}
        className={`${RAIL_BUTTON} border-transparent`}
      >
        <CalendarCheck size={18} />
      </button>
      <button
        type="button"
        onClick={() => onAsk(t('shell.edgeDock.quick.hintPrompt'))}
        title={t('shell.edgeDock.quick.hint')}
        className={`${RAIL_BUTTON} border-transparent`}
      >
        <Lightbulb size={18} />
      </button>
      <button
        type="button"
        onClick={() => saveConfigFlag(PASSENGER_PATH, !passenger)}
        aria-pressed={passenger}
        title={t('shell.edgeDock.quick.passenger')}
        className={`${RAIL_BUTTON} ${passenger ? RAIL_ON : 'border-transparent'}`}
      >
        <Ghost size={18} />
      </button>
    </>
  )
}
