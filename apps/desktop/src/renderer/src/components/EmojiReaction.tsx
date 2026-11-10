import { memo, useCallback, useEffect, useRef, useState } from 'react'

type FloatingEmoji = {
  id: number
  emoji: string
  x: number
  y: number
  angle: number
  speed: number
  size: number
  /** Só o primeiro emoji de cada rajada desenha o anel de toque. */
  ring: boolean
}

const REACTION_EMOJIS = ['💜', '✨', '🌸', '⭐', '💫', '🎀', '🌺', '💖', '🦋', '🌙', '🎉', '🔥', '👾', '🫶']
const BURST_COUNT = 6
const MAX_ACTIVE_REACTIONS = 12

export function useEmojiReactions() {
  const [reactions, setReactions] = useState<FloatingEmoji[]>([])
  const nextId = useRef(0)

  const spawnReaction = useCallback((eventOrX: React.MouseEvent | number, yCoord?: number) => {
    let x: number
    let y: number

    if (typeof eventOrX === 'number' && typeof yCoord === 'number') {
      x = eventOrX
      y = yCoord
    } else {
      const event = eventOrX as React.MouseEvent
      const rect = (event.currentTarget as HTMLElement).getBoundingClientRect()
      x = event.clientX - rect.left
      y = event.clientY - rect.top
    }

    const newEmojis: FloatingEmoji[] = Array.from({ length: BURST_COUNT }, (_, i): FloatingEmoji => {
      const spreadAngle = (360 / BURST_COUNT) * i + (Math.random() - 0.5) * 30
      return {
        id: nextId.current++,
        emoji: REACTION_EMOJIS[Math.floor(Math.random() * REACTION_EMOJIS.length)],
        x,
        y,
        angle: spreadAngle,
        speed: 55 + Math.random() * 45,
        size: 18 + Math.floor(Math.random() * 10),
        ring: i === 0,
      }
    })

    setReactions((prev) => [...prev, ...newEmojis].slice(-MAX_ACTIVE_REACTIONS))
    setTimeout(() => {
      setReactions((prev) => prev.filter((r) => !newEmojis.some((ne) => ne.id === r.id)))
    }, 1400)
  }, [])

  return { reactions, spawnReaction }
}

export const EmojiReactionLayer = memo(function EmojiReactionLayer({ reactions }: { reactions: FloatingEmoji[] }) {
  return (
    <>
      {reactions.map((r) => (
        <FloatingEmojiItem key={r.id} {...r} />
      ))}
    </>
  )
})

const FloatingEmojiItem = memo(function FloatingEmojiItem({ emoji, x, y, angle, speed, size, ring }: FloatingEmoji) {
  const [phase, setPhase] = useState(0)
  const rad = (angle * Math.PI) / 180
  const dx = Math.cos(rad) * speed

  const dy = Math.sin(rad) * speed - speed * 0.6
  const gravity = speed * 0.35

  useEffect(() => {
    const t1 = setTimeout(() => setPhase(1), 16)
    const t2 = setTimeout(() => setPhase(2), 900)
    return () => {
      clearTimeout(t1)
      clearTimeout(t2)
    }
  }, [])

  const translateY = phase === 1 ? dy : phase === 2 ? dy + gravity : 0
  const scale = phase === 0 ? 0 : phase === 1 ? 1.25 : 0.3
  const opacity = phase === 1 ? 1 : 0

  return (
    <>
      {ring && (
        <div
          aria-hidden="true"
          data-testid="touch-ripple"
          className="touch-ripple pointer-events-none absolute size-28 rounded-full border-2 border-accent"
          style={{
            left: x,
            top: y,
            zIndex: 'calc(var(--layer-avatar, 0) + 4)',
            boxShadow: '0 0 18px var(--accent-glow), inset 0 0 14px var(--accent-glow)',
          }}
        />
      )}
      <div
        aria-hidden="true"
        style={{
          position: 'absolute',
          left: x,
          top: y,
          fontSize: size,
          pointerEvents: 'none',
          zIndex: 'calc(var(--layer-avatar, 0) + 5)',
          transform: `translateX(-50%) translate(${phase * dx}px, ${translateY}px) scale(${scale})`,
          opacity,
          transition:
            phase === 1
              ? 'all 0.75s cubic-bezier(0.22, 1.4, 0.36, 1)'
              : 'opacity 0.35s ease-out, transform 0.45s ease-in',
          filter: 'drop-shadow(0 0 6px var(--accent-glow))',
          willChange: 'transform, opacity',
        }}
      >
        {emoji}
      </div>
    </>
  )
})
