import { motion } from 'framer-motion'
import { Zap } from 'lucide-react'
import { memo, useCallback, useEffect, useState } from 'react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { BazaDots, CardView } from './truco-card'
import { playTurn, startTrucoRound, type TrucoGameState } from './truco-engine'
import { TrucoOverlays } from './truco-overlays'

export const TrucoBoard = memo(function TrucoBoard({ initialState }: { initialState: TrucoGameState }) {
  const { t } = useI18n()
  const [state, setState] = useState<TrucoGameState>(initialState)

  useEffect(() => {
    setState(initialState)
  }, [initialState])

  const handlePlayCard = useCallback((cardId: string) => {
    setState((prev) => playTurn(prev, cardId))
  }, [])

  const handleNextHand = useCallback(() => {
    setState((prev) => startTrucoRound(prev.score, prev.taunts, prev.tauntsUnavailable))
  }, [])

  const handleRestartMatch = useCallback(() => {
    setState((prev) => startTrucoRound({ user: 0, mika: 0 }, prev.taunts, prev.tauntsUnavailable))
  }, [])

  const currentBaza = state.bazas[state.bazas.length - 1]

  return (
    <div className="relative flex h-full min-h-[460px] w-full flex-col justify-between rounded-2xl border border-border-subtle bg-bg-surface/90 p-4 text-text-main shadow-2xl overflow-hidden select-none">
      <div className="flex items-center justify-between z-10">
        <div className="flex items-center gap-3 rounded-xl border border-border-subtle bg-surface-raise px-3 py-1.5 backdrop-blur-md">
          <div className="text-xs font-black">
            <span className="text-info font-bold">
              {t('settings.gamerCompanion.truco.scoreYouLabel')} {state.score.user}
            </span>
            <span className="mx-1.5 opacity-40">x</span>
            <span className="text-accent font-bold">
              {state.score.mika} {t('settings.gamerCompanion.truco.scoreMikaLabel')}
            </span>
          </div>
          <div className="h-3 w-px bg-surface-raise-strong" />
          <div className="text-caption font-bold text-warn">
            {t('settings.gamerCompanion.truco.stakeLabel')} {state.stake} {t('settings.gamerCompanion.truco.stakeUnit')}
          </div>
        </div>

        <BazaDots bazas={state.bazas} />
      </div>

      {state.tauntsUnavailable && (
        <p className="z-10 text-center text-caption font-semibold text-text-subtle">
          {t('settings.gamerCompanion.truco.tauntsUnavailable')}
        </p>
      )}

      <div className="flex flex-col items-center gap-2 z-10">
        {state.currentTaunt && (
          <motion.div
            initial={{ opacity: 0, y: 5 }}
            animate={{ opacity: 1, y: 0 }}
            className="max-w-[280px] rounded-full border border-accent-rim/40 bg-accent-soft/60 px-3 py-1 text-center text-xs font-semibold text-accent backdrop-blur-md shadow-md"
          >
            💬 "{state.currentTaunt}"
          </motion.div>
        )}

        <div className="flex gap-2">
          {state.mikaHand.map((card) => (
            <div
              key={card.id}
              className="h-16 w-12 rounded-lg border border-border-subtle bg-surface-raise-strong p-1 shadow-md flex items-center justify-center"
            >
              <Zap size={14} className="text-accent" aria-hidden="true" />
            </div>
          ))}
        </div>
      </div>

      <div className="my-2 flex items-center justify-center gap-8 z-10">
        <div className="flex flex-col items-center gap-1">
          <span className="text-micro font-bold uppercase tracking-wider text-text-muted">
            {t('settings.gamerCompanion.truco.viraLabel')}
          </span>
          <div className="scale-75 origin-center">
            <CardView card={state.vira} isUser={false} disabled />
          </div>
          <span className="rounded-full bg-surface-raise border border-warn px-2 py-0.5 text-caption font-black text-warn">
            {t('settings.gamerCompanion.truco.manilhaLabel')} {state.manilhaRank}
          </span>
        </div>

        <div className="flex items-center gap-4 min-h-[120px] rounded-2xl border border-border-subtle bg-surface-raise/50 p-3 backdrop-blur-sm">
          {currentBaza?.userCard && (
            <div className="flex flex-col items-center gap-1">
              <span className="text-caption font-bold text-info">
                {t('settings.gamerCompanion.truco.yourCardLabel')}
              </span>
              <div className="scale-90 origin-center">
                <CardView card={currentBaza.userCard} isUser={false} disabled />
              </div>
            </div>
          )}

          {currentBaza?.mikaCard && (
            <div className="flex flex-col items-center gap-1">
              <span className="text-caption font-bold text-accent">
                {t('settings.gamerCompanion.truco.mikaCardLabel')}
              </span>
              <div className="scale-90 origin-center">
                <CardView card={currentBaza.mikaCard} isUser={false} disabled />
              </div>
            </div>
          )}

          {!currentBaza && (
            <div className="text-xs font-bold text-text-muted/60 px-6 py-4">
              {t('settings.gamerCompanion.truco.playPrompt')}
            </div>
          )}
        </div>
      </div>

      <div className="flex flex-col items-center gap-2 z-10">
        <div className="flex gap-3">
          {state.userHand.map((card) => (
            <CardView
              key={card.id}
              card={card}
              isUser={true}
              disabled={Boolean(state.roundWinner || state.matchWinner)}
              onClick={() => handlePlayCard(card.id)}
            />
          ))}
        </div>
        <span className="text-caption font-bold text-text-muted">{t('settings.gamerCompanion.truco.playHint')}</span>
      </div>

      <TrucoOverlays state={state} handleNextHand={handleNextHand} handleRestartMatch={handleRestartMatch} t={t} />
    </div>
  )
})
