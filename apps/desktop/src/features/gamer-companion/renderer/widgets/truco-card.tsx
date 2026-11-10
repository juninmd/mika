import { motion } from 'framer-motion'
import { Sparkles } from 'lucide-react'
import { useI18n } from '../../../../renderer/src/hooks/useI18n'
import { type BazaRecord, SUIT_COLORS, SUIT_SYMBOLS, type TrucoCard } from './truco-engine'

export function CardView({
  card,
  isUser,
  onClick,
  disabled,
}: {
  card: TrucoCard
  isUser: boolean
  onClick?: () => void
  disabled?: boolean
}) {
  const suitColor = SUIT_COLORS[card.suit]
  const symbol = SUIT_SYMBOLS[card.suit]

  return (
    <motion.button
      type="button"
      disabled={disabled || !isUser}
      onClick={onClick}
      whileHover={isUser && !disabled ? { y: -8, scale: 1.05 } : {}}
      whileTap={isUser && !disabled ? { scale: 0.95 } : {}}
      className={`relative flex h-28 w-20 flex-col justify-between rounded-xl border-2 p-2 shadow-xl backdrop-blur-md transition duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus ${
        card.isManilha
          ? 'border-warn bg-surface-raise shadow-[0_0_16px_color-mix(in_srgb,var(--warn)_25%,transparent)]'
          : 'border-border-subtle bg-surface-raise hover:border-info-rim hover:bg-surface-raise-strong'
      } ${disabled ? 'cursor-not-allowed opacity-40' : 'cursor-pointer'}`}
    >
      {card.isManilha && (
        <div className="absolute -top-2.5 -right-2.5 rounded-full bg-warn p-1 text-caption font-black text-bg-surface shadow-md">
          <Sparkles size={10} aria-hidden="true" />
        </div>
      )}

      <div className="flex items-center justify-between text-xs font-black" style={{ color: suitColor }}>
        <span>{card.rank}</span>
        <span className="text-sm">{symbol}</span>
      </div>

      <div className="flex flex-col items-center justify-center">
        <span className="text-2xl" style={{ color: suitColor }}>
          {symbol}
        </span>
        <span className="mt-0.5 text-micro font-bold text-text-muted">{card.name}</span>
      </div>

      <div className="flex rotate-180 items-center justify-between text-xs font-black" style={{ color: suitColor }}>
        <span>{card.rank}</span>
        <span className="text-sm">{symbol}</span>
      </div>
    </motion.button>
  )
}

export function BazaDots({ bazas }: { bazas: BazaRecord[] }) {
  const { t } = useI18n()
  const winnerClass = (winner: BazaRecord['winner']) =>
    winner === 'user'
      ? 'bg-info text-bg-surface shadow-[0_0_8px_var(--neural-cyan)] scale-110'
      : winner === 'mika'
        ? 'bg-accent text-bg-surface shadow-[0_0_8px_var(--neural-purple)] scale-110'
        : winner === 'draw'
          ? 'bg-warn text-bg-surface'
          : 'bg-surface-raise-strong text-text-muted border border-border-subtle'

  return (
    <div className="flex items-center gap-1.5 rounded-full border border-border-subtle bg-bg-surface/80 px-3 py-1 text-caption font-bold text-text-muted">
      <span>{t('settings.gamerCompanion.truco.tricksLabel')}</span>
      <div className="flex gap-1.5">
        {[0, 1, 2].map((idx) => {
          const b = bazas[idx]
          const winner = b?.winner
          return (
            <div
              key={idx}
              className={`flex h-4 w-4 items-center justify-center rounded-full text-micro font-black transition ${winnerClass(winner)}`}
            >
              {winner === 'user' ? 'V' : winner === 'mika' ? 'M' : winner === 'draw' ? '=' : idx + 1}
            </div>
          )
        })}
      </div>
    </div>
  )
}
