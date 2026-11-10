import { AnimatePresence, motion } from 'framer-motion'
import { type FocusEvent, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { layerStyle } from '../../lib/layer-styles'

const HOVER_DELAY_MS = 350
const VIEWPORT_MARGIN = 8

type TooltipProps = {
  label?: string
  detail?: string

  visible: boolean
}

export function Tooltip({ label, detail, visible }: TooltipProps) {
  const open = visible && Boolean(label)
  const tooltipRef = useRef<HTMLSpanElement>(null)
  const [placement, setPlacement] = useState<{ xShift: number; placeBelow: boolean }>({
    xShift: 0,
    placeBelow: false,
  })

  // biome-ignore lint/correctness/useExhaustiveDependencies: label/detail are trigger-only deps — they re-measure the tooltip when content changes its rendered size, even though the body reads the DOM rect, not the props.
  useLayoutEffect(() => {
    if (!open || !tooltipRef.current) return
    const rect = tooltipRef.current.getBoundingClientRect()
    let xShift = 0
    let placeBelow = false

    if (rect.left < VIEWPORT_MARGIN) {
      xShift = VIEWPORT_MARGIN - rect.left
    } else if (rect.right > window.innerWidth - VIEWPORT_MARGIN) {
      xShift = window.innerWidth - VIEWPORT_MARGIN - rect.right
    }

    if (rect.top < VIEWPORT_MARGIN) {
      placeBelow = true
    }

    setPlacement({ xShift, placeBelow })
  }, [open, label, detail])

  return (
    <AnimatePresence>
      {open && (
        <motion.span
          ref={tooltipRef}
          role="tooltip"
          initial={{ opacity: 0, y: placement.placeBelow ? -4 : 4, scale: 0.95 }}
          animate={{ opacity: 1, y: 0, scale: 1 }}
          exit={{ opacity: 0, y: placement.placeBelow ? -4 : 4, scale: 0.95 }}
          transition={{ duration: 0.14 }}
          className={`pointer-events-none absolute left-1/2 -translate-x-1/2 whitespace-nowrap z-50 ${
            placement.placeBelow ? 'top-full mt-2' : 'bottom-full mb-2'
          }`}
          style={layerStyle('menu')}
        >
          <span
            className="block max-w-[calc(100vw-16px)] rounded-xl border border-border-subtle bg-popover px-3 py-2 shadow-xl transition-transform"
            style={placement.xShift !== 0 ? { transform: `translateX(${placement.xShift}px)` } : undefined}
          >
            <span className="block truncate text-caption font-semibold text-text-main">{label}</span>
            {detail && <span className="block truncate text-caption text-text-muted mt-0.5">{detail}</span>}
          </span>
          <span
            className={`absolute left-1/2 -translate-x-1/2 border-4 border-transparent ${
              placement.placeBelow ? 'bottom-full border-b-bg-surface/95' : 'top-full border-t-bg-surface/95'
            }`}
          />
        </motion.span>
      )}
    </AnimatePresence>
  )
}

export function useTooltipTrigger(delayMs = HOVER_DELAY_MS) {
  const [visible, setVisible] = useState(false)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const show = useCallback(() => {
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(() => setVisible(true), delayMs)
  }, [delayMs])

  const hide = useCallback(() => {
    if (timer.current) clearTimeout(timer.current)
    setVisible(false)
  }, [])

  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current)
    }
  }, [])

  // A click leaves focus on the button; opening on that focus parked the tooltip over the reply bubble.
  const showOnKeyboardFocus = useCallback(
    (e: FocusEvent<HTMLElement>) => {
      if (e.target instanceof Element && e.target.matches(':focus-visible')) show()
    },
    [show],
  )

  return {
    visible,
    triggerProps: {
      onMouseEnter: show,
      onMouseLeave: hide,
      onFocus: showOnKeyboardFocus,
      onBlur: hide,
      onPointerDown: hide,
    },
  }
}
