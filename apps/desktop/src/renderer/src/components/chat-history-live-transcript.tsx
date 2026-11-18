import { Mic } from 'lucide-react'
import { useEffect, useRef, useSyncExternalStore } from 'react'
import { useI18n } from '../hooks/useI18n'
import { type LiveTranscript, subscribeLiveTranscript } from '../lib/live-transcript-bus'

let snapshot: LiveTranscript = null
const subscribe = (onChange: () => void) =>
  subscribeLiveTranscript((text) => {
    snapshot = text
    onChange()
  })

/** What the user is saying right now, growing as partial STT arrives; gone once the turn lands. */
export function LiveTranscriptBubble() {
  const { t } = useI18n()
  const text = useSyncExternalStore(subscribe, () => snapshot)
  const ref = useRef<HTMLElement>(null)

  useEffect(() => {
    if (text !== null) ref.current?.scrollIntoView({ block: 'nearest' })
  }, [text])

  if (text === null) return null
  // aria-hidden: partials rewrite themselves every second; the log announces the final turn instead.
  return (
    <article
      ref={ref}
      data-testid="live-transcript"
      aria-hidden="true"
      className="flex justify-end animate-in fade-in duration-200"
    >
      <div className="max-w-[85%] rounded-2xl rounded-tr-sm border border-dashed border-info-rim/50 bg-info-soft/30 px-4 py-3 shadow-sm">
        <div className="mb-1.5 flex items-center gap-2 text-micro font-bold uppercase tracking-wider text-info">
          <Mic className="h-3 w-3 animate-pulse" aria-hidden="true" />
          <span>{t('shell.history.liveListening')}</span>
        </div>
        <div className="flex flex-wrap items-center gap-x-2 text-[13px] leading-relaxed text-text-main/80">
          {text && <span className="whitespace-pre-wrap">{text}</span>}
          <span className="inline-flex h-4 items-center gap-1">
            {[0, 1, 2].map((i) => (
              <span
                key={i}
                className="thinking-dot inline-block h-1.5 w-1.5 rounded-full bg-info"
                style={{ animationDelay: `${i * 0.2}s` }}
              />
            ))}
          </span>
        </div>
      </div>
    </article>
  )
}
