// Shared types for the audio playback pipeline. No React import — a plain mutable-box shape
// stands in for React.MutableRefObject so engine modules stay React-free.
import type { WordBoundary } from '../lib/lip-sync-source'

export type Ref<T> = { current: T }

export type QueuedAudio = {
  audio: string
  mimeType: string
  words?: WordBoundary[]
  decoded?: Promise<AudioBuffer | null>
}

export interface PlaybackRefs {
  token: number
  playbackTokenRef: Ref<number>
  playbackResolveRef: Ref<(() => void) | null>
  stopPlaybackRef: Ref<(() => void) | null>
  setIsPlaying: (playing: boolean) => void
  finishPlayback: () => void
}
