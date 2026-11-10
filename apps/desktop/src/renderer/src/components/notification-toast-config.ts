import type { TranslationDict } from '@mika/sdk/i18n'
import type { NotificationKind } from '../store/useUIStore'

export const KIND_CONFIG: Record<NotificationKind, { text: string; label: keyof TranslationDict }> = {
  info: {
    text: 'text-primary',
    label: 'shell.toast.info',
  },
  success: {
    text: 'text-ok',
    label: 'shell.toast.success',
  },
  warning: {
    text: 'text-warn',
    label: 'shell.toast.warning',
  },
  error: {
    text: 'text-danger',
    label: 'shell.toast.error',
  },
}

export const DISMISS_DURATION_S: Record<NotificationKind, number> = {
  info: 3,
  success: 2.5,
  warning: 4.5,
  error: 6,
}

export const PROGRESS_KEYFRAMES =
  '@keyframes toast-progress { from { transform: scaleX(1) } to { transform: scaleX(0) } }'
