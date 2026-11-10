import { AnimatePresence, motion } from 'framer-motion'
import type { RefObject } from 'react'
import type { WheelItem } from './use-neural-wheel-items'

function getItemColorClass(color: WheelItem['color']): string {
  switch (color) {
    case 'cyan':
      return 'text-info'
    case 'purple':
      return 'text-accent'
    case 'mint':
      return 'text-ok'
    case 'coral':
      return 'text-danger'
    case 'amber':
      return 'text-warn'
    default:
      return 'text-info'
  }
}

export function NeuralWheelItemButton({
  item,
  index: i,
  x,
  y,
  isActive,
  isGamesView,
  setActiveIndex,
  pointerHandledRef,
  executeItem,
}: {
  item: WheelItem
  index: number
  x: number
  y: number
  isActive: boolean
  isGamesView: boolean
  setActiveIndex: (index: number | null) => void
  pointerHandledRef: RefObject<boolean>
  executeItem: (item: WheelItem) => void
}) {
  return (
    <motion.button
      type="button"
      data-testid={`neural-wheel-item-${item.id}`}
      aria-label={item.label}
      aria-haspopup={item.kind === 'submenu' ? 'menu' : undefined}
      aria-expanded={item.kind === 'submenu' ? isGamesView : undefined}
      onMouseEnter={() => setActiveIndex(i)}
      onMouseLeave={() => setActiveIndex(null)}
      onFocus={() => setActiveIndex(i)}
      onBlur={() => setActiveIndex(null)}
      onPointerDown={(e) => {
        e.stopPropagation()
        pointerHandledRef.current = true
        executeItem(item)
      }}
      onClick={() => {
        if (pointerHandledRef.current) {
          pointerHandledRef.current = false
          return
        }
        executeItem(item)
      }}
      className="absolute flex items-center justify-center group rounded-2xl focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      style={
        {
          left: x,
          top: y,
          transform: 'translate(-50%, -50%)',
          WebkitAppRegion: 'no-drag',
        } as React.CSSProperties
      }
      whileHover={{ scale: 1.2 }}
      whileTap={{ scale: 0.9 }}
    >
      <div
        className={`w-16 h-16 rounded-2xl flex items-center justify-center transition duration-300 border-2 ${
          isActive
            ? 'bg-accent text-bg-surface border-accent-rim shadow-lg shadow-accent/40'
            : `bg-bg-surface/90 ${getItemColorClass(item.color)} border-border-subtle hover:border-border-strong hover:bg-surface-raise shadow-xl`
        }`}
      >
        {item.icon}
      </div>

      {/* Every node names itself: eight icons asked the user to hover each one to learn what it does. */}
      {!isActive && (
        <span
          aria-hidden="true"
          className="pointer-events-none absolute -bottom-6 whitespace-nowrap text-micro font-semibold text-text-main drop-shadow-[0_1px_4px_rgba(0,0,0,0.9)]"
        >
          {item.label}
        </span>
      )}

      <AnimatePresence>
        {isActive && (
          <motion.div
            aria-hidden="true"
            initial={{ opacity: 0, y: 10 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 5 }}
            className="absolute -bottom-10 whitespace-nowrap px-3 py-1 bg-accent text-bg-surface font-black uppercase text-micro rounded-md tracking-tighter shadow-md"
          >
            {item.label}
          </motion.div>
        )}
      </AnimatePresence>
    </motion.button>
  )
}
