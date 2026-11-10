export type TestStatus = 'idle' | 'testing' | 'ok' | 'error'

export interface TestResultNoteProps {
  status: TestStatus
  message: string
  className?: string
}

export function TestResultNote({ status, message, className = '' }: TestResultNoteProps) {
  if (!message || status === 'idle') return null
  const tone =
    status === 'ok'
      ? 'border-ok-rim bg-ok-soft text-ok'
      : status === 'testing'
        ? 'border-info-rim bg-info-soft text-info'
        : 'border-danger-rim bg-danger-soft text-danger'

  return (
    <p
      role={status === 'error' ? 'alert' : 'status'}
      aria-live="polite"
      className={`rounded-lg border px-3 py-2 text-caption leading-relaxed ${tone} ${className}`}
    >
      {message}
    </p>
  )
}
