import { useEffect, useState } from 'react'
import { getFriendlyToolLabel } from '../lib/tool-labels'
import type { ActiveToolState } from './app/app-shell.types'

export function ThinkingDots({ activeTool }: { activeTool?: ActiveToolState }) {
  const [elapsedMs, setElapsedMs] = useState(0)

  useEffect(() => {
    if (!activeTool) {
      setElapsedMs(0)
      return
    }
    if (activeTool.finishedAt) {
      setElapsedMs(activeTool.durationMs ?? activeTool.finishedAt - activeTool.startedAt)
      return
    }
    setElapsedMs(Date.now() - activeTool.startedAt)
    const id = setInterval(() => setElapsedMs(Date.now() - activeTool.startedAt), 200)
    return () => clearInterval(id)
  }, [
    activeTool?.name,
    activeTool?.startedAt,
    activeTool?.finishedAt,
    activeTool?.durationMs,
    activeTool?.success,
    activeTool,
  ])

  let icon = '⚙'
  let tone = 'text-info/80'

  if (activeTool?.finishedAt) {
    if (activeTool.success === false) {
      icon = '✗'
      tone = 'text-danger/90'
    } else {
      icon = '✓'
      tone = 'text-ok/90'
    }
  }

  return (
    <div className="flex flex-col items-center gap-1">
      <div className="flex h-6 items-center justify-center gap-2" aria-hidden="true">
        {[0, 1, 2].map((i) => (
          <span
            key={i}
            className="thinking-dot inline-block h-2 w-2 rounded-full"
            style={{ backgroundColor: 'var(--neural-cyan)', animationDelay: `${i * 0.2}s` }}
          />
        ))}
      </div>
      {activeTool && (
        <span className={`flex items-center gap-1 font-mono text-caption tracking-wide ${tone}`}>
          <span aria-hidden="true">{icon}</span>
          <span className="max-w-[16rem] truncate">{getFriendlyToolLabel(activeTool.name)}</span>
          <span>· {(elapsedMs / 1000).toFixed(1)}s</span>
        </span>
      )}
    </div>
  )
}
