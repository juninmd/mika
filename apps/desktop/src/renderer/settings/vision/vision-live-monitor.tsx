import { useEffect, useRef, useState } from 'react'
import { useI18n } from '../../src/hooks/useI18n'
import { getElectronApi } from '../../src/lib/shell-api'
import { MAX_HISTORY, type Observation, observationFromPayload } from './vision-monitor-model'
import { LatestObservation, VisionStatusBar } from './vision-monitor-observation'
import { EmptyObservation, ObservationHistory, VisionSignalGrid } from './vision-monitor-signals'

export function VisionLiveMonitor({ blockedReason }: { blockedReason?: string }) {
  const { t } = useI18n()
  const [observations, setObservations] = useState<Observation[]>([])
  const [totalCount, setTotalCount] = useState(0)
  const [active, setActive] = useState(false)
  const fadeRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const observationsRef = useRef<Observation[]>([])
  const lastAtRef = useRef(0)
  const bucketRef = useRef(0)
  const [, forceRender] = useState(0)

  useEffect(() => {
    let interval: ReturnType<typeof setInterval> | null = null
    const tick = () => {
      const lastAt = lastAtRef.current
      if (!lastAt) return
      const diff = Math.floor((Date.now() - lastAt) / 1000)
      const bucket = diff < 60 ? Math.floor(diff / 5) : Math.floor(diff / 60) + 12
      if (bucket !== bucketRef.current) {
        bucketRef.current = bucket
        forceRender((n) => n + 1)
      }
    }
    const start = () => {
      if (interval !== null) return
      interval = setInterval(tick, 1000)
    }
    const stop = () => {
      if (interval !== null) clearInterval(interval)
      interval = null
    }
    const onVisibilityChange = () => {
      if (document.hidden) stop()
      else start()
    }
    document.addEventListener('visibilitychange', onVisibilityChange)
    if (!document.hidden) start()
    return () => {
      stop()
      document.removeEventListener('visibilitychange', onVisibilityChange)
    }
  }, [])

  useEffect(() => {
    const eAPI = getElectronApi()
    if (!eAPI?.runtime?.onScreenObservation) return

    const unsub = eAPI.runtime.onScreenObservation((payload) => {
      if (fadeRef.current) clearTimeout(fadeRef.current)
      setActive(true)

      const now = Date.now()
      const prev = observationsRef.current
      const existingIndex = prev.findIndex((obs) => {
        const isSameApp =
          obs.foregroundApp?.title === payload.foregroundApp?.title &&
          obs.foregroundApp?.processName === payload.foregroundApp?.processName
        const isRecent = now - obs.at < 6000
        return isSameApp && isRecent
      })

      const obs = observationFromPayload(payload)
      const next =
        existingIndex !== -1
          ? prev.map((o, i) => (i === existingIndex ? { ...o, ...obs, at: o.at } : o))
          : [obs, ...prev].slice(0, MAX_HISTORY)

      if (existingIndex === -1) setTotalCount((n) => n + 1)
      observationsRef.current = next
      setObservations(next)
      lastAtRef.current = now
      bucketRef.current = 0

      fadeRef.current = setTimeout(() => setActive(false), 3000)
    })

    return () => {
      unsub()
      if (fadeRef.current) clearTimeout(fadeRef.current)
    }
  }, [])

  const latest = observations[0]

  return (
    <div className="space-y-3">
      <VisionStatusBar active={active} hasObservations={observations.length > 0} totalCount={totalCount} t={t} />
      {latest && <LatestObservation observation={latest} active={active} t={t} />}
      {latest && <VisionSignalGrid observation={latest} t={t} />}
      <ObservationHistory observations={observations.slice(1)} t={t} />
      {observations.length === 0 && <EmptyObservation t={t} blockedReason={blockedReason} />}
    </div>
  )
}
