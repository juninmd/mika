import { MOTION } from '@mika/sdk-ui/settings'
import { AnimatePresence, motion } from 'framer-motion'
import { memo, useEffect, useRef, useState } from 'react'
import { useI18n } from '../hooks/useI18n'
import { type AppNotification, useUIStore } from '../store/useUIStore'
import type { HudRect } from './hud/hud-layout'
import { DISMISS_DURATION_S, KIND_CONFIG, PROGRESS_KEYFRAMES } from './notification-toast-config'

function XIcon() {
  return (
    <svg
      width="14"
      height="14"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="3"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <line x1="18" y1="6" x2="6" y2="18" />
      <line x1="6" y1="6" x2="18" y2="18" />
    </svg>
  )
}

interface NotificationToastProps {
  notification: AppNotification
  onDismiss: (id: string) => void
}

function NotificationToastItem({ notification, onDismiss }: NotificationToastProps) {
  const { t } = useI18n()
  const config = KIND_CONFIG[notification.kind] ?? KIND_CONFIG.info
  const duration = DISMISS_DURATION_S[notification.kind] ?? 4
  const [isHovered, setIsHovered] = useState(false)
  const remainingMsRef = useRef(duration * 1000)
  const startTimestampRef = useRef(Date.now())

  useEffect(() => {
    useUIStore.getState().markNotificationShown(notification.id)
  }, [notification.id])

  useEffect(() => {
    if (isHovered) return
    startTimestampRef.current = Date.now()
    const timer = setTimeout(() => {
      onDismiss(notification.id)
    }, remainingMsRef.current)

    return () => {
      clearTimeout(timer)
      const elapsed = Date.now() - startTimestampRef.current
      remainingMsRef.current = Math.max(500, remainingMsRef.current - elapsed)
    }
  }, [isHovered, notification.id, onDismiss])

  return (
    <motion.div
      layout
      {...MOTION.toast}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onFocus={() => setIsHovered(true)}
      onBlur={() => setIsHovered(false)}
      role={notification.kind === 'error' || notification.kind === 'warning' ? 'alert' : 'status'}
      className="hud-glass-strong relative flex w-full shrink-0 overflow-hidden rounded-xl"
    >
      <span
        aria-hidden="true"
        className={`absolute inset-y-2 left-0 w-[3px] rounded-r-full bg-current ${config.text}`}
      />
      <div className="min-w-0 flex-1 py-2 pl-3.5 pr-1">
        <div className="flex items-baseline gap-2">
          <span className={`shrink-0 text-micro font-bold uppercase tracking-[0.12em] ${config.text}`}>
            {t(config.label)}
          </span>
          {notification.title && (
            <span className="min-w-0 truncate text-caption font-semibold text-text-main" title={notification.title}>
              {notification.title}
            </span>
          )}
        </div>
        <p
          title={notification.message}
          className="mt-0.5 line-clamp-2 break-words text-caption leading-snug text-text-muted [overflow-wrap:anywhere]"
        >
          {notification.message}
        </p>
      </div>
      <button
        type="button"
        onClick={() => onDismiss(notification.id)}
        aria-label={`${t('shell.toast.close')}: ${notification.title ?? t(config.label)}`}
        className="flex h-8 w-8 shrink-0 items-center justify-center self-start rounded-lg text-text-muted transition-colors hover:bg-surface-raise hover:text-text-main focus:outline-none focus-visible:ring-2 focus-visible:ring-focus"
      >
        <XIcon />
      </button>
      <div aria-hidden="true" className="absolute inset-x-0 bottom-0 h-[2px] overflow-hidden">
        <div
          className={`h-full origin-left bg-current opacity-70 ${config.text}`}
          style={{
            animation: `toast-progress ${duration}s linear forwards`,
            animationPlayState: isHovered ? 'paused' : 'running',
          }}
        />
      </div>
    </motion.div>
  )
}

interface NotificationContainerProps {
  notifications: AppNotification[]
  onDismiss: (id: string) => void
  /** Toast column from `hud-layout`: right side, below the bubble zone, clear of her face. */
  anchor: HudRect
}

const MAX_VISIBLE_TOASTS = 2

export const NotificationContainer = memo(function NotificationContainer({
  notifications,
  onDismiss,
  anchor,
}: NotificationContainerProps) {
  const { t } = useI18n()
  if (notifications.length === 0) return null
  const visible = notifications.slice(-MAX_VISIBLE_TOASTS)
  const queued = notifications.length - visible.length

  return (
    <>
      <style>{PROGRESS_KEYFRAMES}</style>
      <div
        data-testid="notification-toasts"
        aria-live="polite"
        aria-atomic="false"
        className="pointer-events-auto fixed flex flex-col gap-2 overflow-y-auto"
        style={{
          zIndex: 'var(--layer-toasts)',
          left: anchor.x,
          top: anchor.y,
          width: anchor.width,
          maxHeight: Math.max(anchor.height, 96),
        }}
      >
        {queued > 0 && (
          <button
            type="button"
            data-testid="notification-toasts-overflow"
            onClick={() => useUIStore.getState().dismissAllNotifications()}
            title={t('shell.toast.dismissAllAria')}
            aria-label={t('shell.toast.dismissAllAria')}
            className="self-end rounded border border-border-subtle bg-bg-surface/95 px-2.5 py-1.5 font-mono text-micro font-bold uppercase tracking-[0.2em] text-text-muted transition-colors hover:border-danger-rim hover:bg-danger-soft hover:text-danger cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {t('shell.toast.dismissAll')} (+{queued}{' '}
            {queued === 1 ? t('shell.toast.queuedOne') : t('shell.toast.queuedMany')})
          </button>
        )}
        <AnimatePresence mode="popLayout">
          {visible.map((n) => (
            <NotificationToastItem key={n.id} notification={n} onDismiss={onDismiss} />
          ))}
        </AnimatePresence>
      </div>
    </>
  )
})
