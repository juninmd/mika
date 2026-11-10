import { useEffect, useState } from 'react'
import { getMicLevel, subscribeMicLevel } from '../lib/mic-level-bus'

/**
 * Whether the VAD gate is open, as React state that only changes when the boolean flips — the
 * 10 Hz level itself stays out of React (the presence ring reads it through a CSS variable).
 */
export function useMicHearing(micEnabled: boolean): boolean {
  const [hearing, setHearing] = useState(false)
  useEffect(() => {
    if (!micEnabled) {
      setHearing(false)
      return
    }
    setHearing(getMicLevel().hearing)
    return subscribeMicLevel((sample) => setHearing(sample.hearing))
  }, [micEnabled])
  return hearing
}
