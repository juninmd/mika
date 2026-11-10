import type { ActivityChannel, ActivityPayload, ActivityState } from '@mika/sdk/contracts'
import { useEffect, useState } from 'react'
import { getElectronApi } from '../lib/shell-api'

type ActivitySnapshot = Record<ActivityChannel, { state: ActivityState; detail?: string; at: number }>

const INITIAL: ActivitySnapshot = {
  voice: { state: 'idle', at: 0 },
  vision: { state: 'idle', at: 0 },
}

// Activity state from main (the source, not a reconstruction); channels are independent — she can speak and watch the screen at once.
export function useActivity(): ActivitySnapshot {
  const [activity, setActivity] = useState<ActivitySnapshot>(INITIAL)

  useEffect(() => {
    const api = getElectronApi()
    if (!api?.runtime?.onActivity) return

    return api.runtime.onActivity((payload: ActivityPayload) => {
      setActivity((prev) => ({
        ...prev,
        [payload.channel]: { state: payload.state, detail: payload.detail, at: payload.at },
      }))
    })
  }, [])

  return activity
}
