import { AnimatePresence, motion } from 'framer-motion'
import { Trophy } from 'lucide-react'
import type { Translate } from '../../../../renderer/src/hooks/useI18n'
import type { TrucoGameState } from './truco-engine'

export function TrucoOverlays({
  state,
  handleNextHand,
  handleRestartMatch,
  t,
}: {
  state: TrucoGameState
  handleNextHand: () => void
  handleRestartMatch: () => void
  t: Translate
}) {
  return (
    <AnimatePresence>
      {state.roundWinner && !state.matchWinner && (
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0 }}
          className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-bg-surface/95 p-6 backdrop-blur-md text-center"
        >
          <div className="text-2xl font-black mb-1">
            {state.roundWinner === 'user'
              ? t('settings.gamerCompanion.truco.roundWinUser')
              : t('settings.gamerCompanion.truco.roundWinMika')}
          </div>
          <div className="text-xs font-semibold text-text-muted mb-4">
            +{state.stake} {t('settings.gamerCompanion.truco.pointsToLabel')}{' '}
            {state.roundWinner === 'user'
              ? t('settings.gamerCompanion.truco.recipientYou')
              : t('settings.gamerCompanion.truco.recipientMika')}
          </div>
          <button
            type="button"
            onClick={handleNextHand}
            className="rounded-xl border border-info-rim bg-info-soft px-5 py-2.5 text-xs font-black text-info hover:bg-info/25 shadow-lg transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {t('settings.gamerCompanion.truco.nextHandButton')}
          </button>
        </motion.div>
      )}

      {state.matchWinner && (
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0 }}
          className="absolute inset-0 z-40 flex flex-col items-center justify-center bg-bg-surface/95 p-6 backdrop-blur-lg text-center"
        >
          <Trophy size={48} className="text-warn mb-3 animate-bounce" aria-hidden="true" />
          <div className="text-2xl font-black mb-1 text-warn">
            {state.matchWinner === 'user'
              ? t('settings.gamerCompanion.truco.matchWinUser')
              : t('settings.gamerCompanion.truco.matchWinMika')}
          </div>
          <div className="text-sm font-mono font-bold text-text-muted mb-6">
            {t('settings.gamerCompanion.truco.finalScoreLabel')} {t('settings.gamerCompanion.truco.scoreYouLabel')}{' '}
            {state.score.user} x {state.score.mika} {t('settings.gamerCompanion.truco.scoreMikaLabel')}
          </div>
          <button
            type="button"
            onClick={handleRestartMatch}
            className="rounded-xl border border-warn bg-warn/20 px-6 py-3 text-sm font-black text-warn hover:bg-warn/30 shadow-2xl transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {t('settings.gamerCompanion.truco.rematchButton')}
          </button>
        </motion.div>
      )}
    </AnimatePresence>
  )
}
