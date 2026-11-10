import { useEffect } from 'react'

export function useVisiblePolling(run: (silent: boolean) => void, intervalMs: number): void {
  useEffect(() => {
    run(false)

    let interval: ReturnType<typeof setInterval> | null = null
    const start = () => {
      if (interval === null) interval = setInterval(() => run(true), intervalMs)
    }
    const stop = () => {
      if (interval !== null) clearInterval(interval)
      interval = null
    }

    const onVisibilityChange = () => (document.hidden ? stop() : start())
    document.addEventListener('visibilitychange', onVisibilityChange)
    if (!document.hidden) start()

    return () => {
      stop()
      document.removeEventListener('visibilitychange', onVisibilityChange)
    }
  }, [run, intervalMs])
}
