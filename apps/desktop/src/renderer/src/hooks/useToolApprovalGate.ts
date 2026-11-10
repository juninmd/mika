import { t } from '@mika/sdk/i18n'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useEffect, useRef, useState } from 'react'
import { rendererLog } from '../lib/renderer-logger'
import { getElectronApi } from '../lib/shell-api'

import { useChatStore } from '../store/useChatStore'
import { useUIStore } from '../store/useUIStore'

export interface PendingApproval {
  toolCallId: string
  toolName: string
  args: Record<string, unknown>
}

export type ApprovalScope = 'once' | 'session' | 'always'

export function useToolApprovalGate(): {
  pending: PendingApproval | null
  approve: (scope?: ApprovalScope) => void
  deny: () => void
} {
  const [pending, setPending] = useState<PendingApproval | null>(null)
  const queueRef = useRef<PendingApproval[]>([])
  const currentRef = useRef<PendingApproval | null>(null)

  const showNext = useCallback(() => {
    const next = queueRef.current.shift() ?? null
    currentRef.current = next
    setPending(next)
    if (next) {
      useChatStore.getState().setAppStatus('confirming')
    } else {
      useChatStore.getState().setAppStatus((s) => (s === 'confirming' ? 'acting' : s))
    }
  }, [])

  const enqueue = useCallback(
    (entry: PendingApproval) => {
      queueRef.current.push(entry)
      if (currentRef.current === null) showNext()
    },
    [showNext],
  )

  useEffect(() => {
    let unsubscribe: (() => void) | null = null

    try {
      const api = getElectronApi()

      if (api?.runtime?.onToolApprovalRequest) {
        unsubscribe = api.runtime.onToolApprovalRequest((payload) => {
          enqueue({
            toolCallId: payload.toolCallId,
            toolName: payload.toolName,
            args: payload.args,
          })
        })
      }
    } catch (err) {
      rendererLog.error('Failed to set up tool approval gate:', toDisplayError(err, String(err)))
    }

    return () => {
      if (unsubscribe) unsubscribe()
    }
  }, [enqueue])

  const sendResponse = useCallback(
    (approved: boolean, scope: ApprovalScope = 'once') => {
      const current = currentRef.current
      if (!current) return

      try {
        const api = getElectronApi()
        if (api?.runtime?.sendToolApprovalResponse) {
          api.runtime.sendToolApprovalResponse(current.toolCallId, approved, scope)
        }
      } catch (err) {
        rendererLog.error(`Failed to send ${approved ? 'approval' : 'denial'} response`, {
          error: toDisplayError(err, String(err)),
        })
        queueRef.current.unshift(current)
        useUIStore.getState().addNotification('error', t('shell.approval.sendFailed'), t('shell.approval.title'))
      }

      showNext()
    },
    [showNext],
  )

  const approve = useCallback((scope: ApprovalScope = 'once') => sendResponse(true, scope), [sendResponse])

  const deny = useCallback(() => sendResponse(false), [sendResponse])

  return {
    pending,
    approve,
    deny,
  }
}
