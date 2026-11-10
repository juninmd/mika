import { useCallback, useEffect, useRef, useState } from 'react'

export function useTimeoutRef() {
  const ref = useRef<ReturnType<typeof setTimeout> | null>(null)

  const set = useCallback((callback: () => void, ms: number) => {
    if (ref.current) clearTimeout(ref.current)
    ref.current = setTimeout(callback, ms)
  }, [])

  const clear = useCallback(() => {
    if (ref.current) {
      clearTimeout(ref.current)
      ref.current = null
    }
  }, [])

  useEffect(() => clear, [clear])

  return { set, clear }
}

export function useTransientFlag(ms: number): [boolean, () => void, () => void] {
  const [flag, setFlag] = useState(false)
  const { set: scheduleReset, clear: cancelReset } = useTimeoutRef()

  const reset = useCallback(() => {
    cancelReset()
    setFlag(false)
  }, [cancelReset])

  const trigger = useCallback(() => {
    setFlag(true)

    scheduleReset(() => setFlag(false), ms)
  }, [ms, scheduleReset])

  return [flag, trigger, reset]
}
