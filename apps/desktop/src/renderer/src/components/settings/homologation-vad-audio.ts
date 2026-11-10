import type { MutableRefObject } from 'react'

export function getAverageVolume(dataArray: Uint8Array): number {
  let values = 0
  for (let i = 0; i < dataArray.length; i++) {
    values += dataArray[i]
  }
  return values / dataArray.length
}

export interface VadState {
  speakingStreak: number
  silenceStreak: number
  isSpeaking: boolean
}

export function handleVadState(
  normalized: number,
  threshold: number,
  state: VadState,
  onStartSpeech: () => void,
  onStopSpeech: () => void,
): VadState {
  let nextSpeakingStreak = state.speakingStreak
  let nextSilenceStreak = state.silenceStreak
  let nextIsSpeaking = state.isSpeaking

  if (normalized > threshold) {
    nextSpeakingStreak++
    nextSilenceStreak = 0
    if (nextSpeakingStreak >= 4 && !state.isSpeaking) {
      onStartSpeech()
      nextIsSpeaking = true
    }
  } else {
    nextSilenceStreak++
    nextSpeakingStreak = 0
    if (nextSilenceStreak >= 25 && state.isSpeaking) {
      onStopSpeech()
      nextIsSpeaking = false
    }
  }

  return {
    speakingStreak: nextSpeakingStreak,
    silenceStreak: nextSilenceStreak,
    isSpeaking: nextIsSpeaking,
  }
}

interface VadTeardownRefs {
  animationFrameRef: MutableRefObject<number | null>
  mediaRecorderRef: MutableRefObject<MediaRecorder | null>
  audioContextRef: MutableRefObject<AudioContext | null>
}

export function teardownVadResources(refs: VadTeardownRefs, stream: MediaStream | null): void {
  if (refs.animationFrameRef.current) {
    cancelAnimationFrame(refs.animationFrameRef.current)
    refs.animationFrameRef.current = null
  }

  if (refs.mediaRecorderRef.current && refs.mediaRecorderRef.current.state !== 'inactive') {
    try {
      refs.mediaRecorderRef.current.stop()
    } catch (_e) {}
  }

  if (stream) {
    for (const track of stream.getTracks()) {
      track.stop()
    }
  }

  if (refs.audioContextRef.current) {
    refs.audioContextRef.current.close().catch(() => {})
    refs.audioContextRef.current = null
  }
}
