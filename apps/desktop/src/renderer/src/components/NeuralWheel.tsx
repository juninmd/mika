import { AnimatePresence, motion } from 'framer-motion'
import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { useFocusTrap } from '../hooks/useFocusTrap'
import { useI18n } from '../hooks/useI18n'
import { layerStyle } from '../lib/layer-styles'
import { NeuralWheelGames } from './NeuralWheelGames'
import { NeuralWheelHub } from './NeuralWheelHub'
import { NeuralWheelItemButton } from './NeuralWheelItemButton'
import { useNeuralWheelItems, type WheelItem } from './use-neural-wheel-items'
import { useNeuralWheelView, type WheelView } from './use-neural-wheel-view'

const MAX_RADIUS = 160
const BUTTON_MARGIN = 56

export const NeuralWheel = memo(function NeuralWheel({
  isOpen,
  onClose,
  position,
  onCommand,
  micEnabled = false,
  onToggleMic,
  pinned = false,
  onTogglePin,
  onOpenChat,
  onClearChat,
  ttsIsPlaying = false,
  onStopTTS,
}: {
  isOpen: boolean
  onClose: () => void
  position: { x: number; y: number }
  onCommand: (prompt: string) => void
  micEnabled?: boolean
  onToggleMic?: () => void
  pinned?: boolean
  onTogglePin?: () => void
  onOpenChat?: () => void
  onClearChat?: () => void
  ttsIsPlaying?: boolean
  onStopTTS?: () => void
}) {
  const { t } = useI18n()
  const [activeIndex, setActiveIndex] = useState<number | null>(null)
  const [view, setView] = useState<WheelView>('root')
  const [viewportSize, setViewportSize] = useState(() => Math.min(window.innerWidth, window.innerHeight))
  const dialogRef = useRef<HTMLDivElement>(null)
  const handleDialogTab = useFocusTrap(dialogRef)
  // O `whileTap` encolhe o botão entre o pointerdown e o click, então um clique nem sempre fecha no
  // mesmo alvo. Executa no pointerdown e suprime o click que vem depois; teclado (Enter/Espaço) NÃO
  // passa por pointerdown, então cai no click e continua funcionando.
  const pointerHandledRef = useRef(false)

  const items = useNeuralWheelItems({
    onClose,
    onCommand,
    micEnabled,
    onToggleMic,
    pinned,
    onTogglePin,
    onOpenChat,
    onClearChat,
    ttsIsPlaying,
    onStopTTS,
  })

  useEffect(() => {
    const onResize = () => setViewportSize(Math.min(window.innerWidth, window.innerHeight))
    window.addEventListener('resize', onResize)
    return () => window.removeEventListener('resize', onResize)
  }, [])

  useNeuralWheelView(isOpen, view, setView, setActiveIndex, dialogRef, onClose)

  // A pointerdown that closed the wheel may never see its click (the button unmounts first): without this
  // reset the flag stayed set and the next keyboard activation (Enter/Space, which has no pointerdown) was swallowed.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `isOpen` is the trigger, not a value read inside.
  useEffect(() => {
    pointerHandledRef.current = false
  }, [isOpen])

  const executeItem = useCallback(
    (item: WheelItem) => {
      if (item.kind === 'submenu') {
        setView('games')
        setActiveIndex(null)
        return
      }
      if (item.onExecute) {
        item.onExecute()
        return
      }
      if (item.prompt) {
        onCommand(item.prompt)
        onClose()
      }
    },
    [onCommand, onClose],
  )

  const RADIUS = Math.min(MAX_RADIUS, viewportSize / 2 - BUTTON_MARGIN)
  const accentColor = 'var(--neural-cyan)'
  const isGamesView = view === 'games'
  const activeItem = activeIndex === null ? null : (items[activeIndex] ?? null)

  // Clamp the wheel's anchor to the click point so it never opens cut off at the viewport edges.
  const wheelMargin = RADIUS + BUTTON_MARGIN
  const anchorX = Math.min(Math.max(position.x, wheelMargin), Math.max(wheelMargin, window.innerWidth - wheelMargin))
  const anchorY = Math.min(Math.max(position.y, wheelMargin), Math.max(wheelMargin, window.innerHeight - wheelMargin))

  return (
    <AnimatePresence>
      {isOpen && (
        <div
          ref={dialogRef}
          role="dialog"
          aria-modal="true"
          aria-label={t('shell.wheel.ariaLabel')}
          className="fixed inset-0 bg-black/40 pointer-events-auto"
          style={layerStyle('overlay')}
          onKeyDown={handleDialogTab}
          onMouseDown={(e) => {
            if (e.target === e.currentTarget) onClose()
          }}
          onContextMenu={(e) => e.preventDefault()}
        >
          <div
            className="absolute flex items-center justify-center"
            style={{ left: anchorX, top: anchorY, transform: 'translate(-50%, -50%)' }}
          >
            <motion.div
              initial={{ scale: 0, rotate: -180, opacity: 0 }}
              animate={{ scale: 1, rotate: 0, opacity: 1 }}
              exit={{ scale: 0, rotate: 180, opacity: 0 }}
              transition={{ type: 'spring', damping: 20, stiffness: 300 }}
              className="relative w-1 h-1 flex items-center justify-center"
            >
              <NeuralWheelHub
                isGamesView={isGamesView}
                accentColor={accentColor}
                activeItem={activeItem}
                onBack={() => setView('root')}
              />

              {isGamesView && (
                <NeuralWheelGames active={isOpen && isGamesView} accentColor={accentColor} onLaunched={onClose} />
              )}

              {!isGamesView &&
                items.map((item, i) => {
                  const angle = (i * 360) / items.length
                  const radian = (angle * Math.PI) / 180
                  const x = Math.cos(radian) * RADIUS
                  const y = Math.sin(radian) * RADIUS

                  const isActive = activeIndex === i

                  return (
                    <NeuralWheelItemButton
                      key={item.id}
                      item={item}
                      index={i}
                      x={x}
                      y={y}
                      isActive={isActive}
                      isGamesView={isGamesView}
                      setActiveIndex={setActiveIndex}
                      pointerHandledRef={pointerHandledRef}
                      executeItem={executeItem}
                    />
                  )
                })}

              {!isGamesView && (
                <svg
                  aria-hidden="true"
                  className="absolute inset-0 w-[500px] h-[500px] -translate-x-1/2 -translate-y-1/2 pointer-events-none opacity-20 text-border-strong"
                >
                  <title>{t('shell.wheel.connectionLines')}</title>
                  {items.map((item, i) => {
                    const angle = (i * 360) / items.length
                    const radian = (angle * Math.PI) / 180
                    const x2 = 250 + Math.cos(radian) * RADIUS
                    const y2 = 250 + Math.sin(radian) * RADIUS
                    return (
                      <line key={item.id} x1="250" y1="250" x2={x2} y2={y2} stroke="currentColor" strokeWidth="2" />
                    )
                  })}
                </svg>
              )}
            </motion.div>
          </div>
        </div>
      )}
    </AnimatePresence>
  )
})
