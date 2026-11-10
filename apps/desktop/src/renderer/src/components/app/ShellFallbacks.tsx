import { useI18n } from '../../hooks/useI18n'
import { SparkIcon } from '../Icons'

type ShellFallbackProps = {
  title: string
  message: string
  actionLabel?: string
  onAction?: () => void
  busy?: boolean
  error?: Error
  showStack?: boolean
}

export function ShellFallback({
  title,
  message,
  actionLabel,
  onAction,
  busy = false,
  error,
  showStack = false,
}: ShellFallbackProps) {
  const { t } = useI18n()
  const isDev = process.env.NODE_ENV === 'development'

  return (
    <main
      className="flex h-full min-h-screen items-center justify-center bg-bg-deep bg-[radial-gradient(circle_at_top_left,var(--accent-soft),transparent_36%)] p-4 text-text-main"
      aria-busy={busy}
    >
      <section className="w-full max-w-xl rounded-3xl border border-border-subtle bg-bg-surface/90 p-8 text-center shadow-2xl shadow-black/40">
        <div className="mx-auto inline-flex w-fit items-center gap-1.5 rounded-md border border-border-subtle bg-bg-deep px-2.5 py-1 text-micro font-extrabold uppercase tracking-widest text-primary">
          <SparkIcon className="size-3.5" aria-hidden="true" />
          Mika Desktop
        </div>
        <h1 className="mt-5 font-display text-3xl font-bold tracking-tight text-text-main">{title}</h1>
        <p className="mx-auto mt-3 max-w-md text-sm leading-7 text-text-muted">{message}</p>

        {showStack && error && isDev ? (
          <details className="mx-auto mt-4 max-w-sm rounded-lg border border-danger-rim bg-danger-soft p-3 text-left">
            <summary className="cursor-pointer font-mono text-xs font-bold text-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded">
              {t('shell.fallback.errorDetails')}
            </summary>
            <pre className="mt-2 max-h-48 overflow-auto rounded bg-surface-raise/40 p-2 font-mono text-caption leading-tight text-danger">
              {error.stack}
            </pre>
          </details>
        ) : null}

        {busy ? (
          <div
            className="mx-auto mt-6 h-2 w-full max-w-xs overflow-hidden rounded-full bg-surface-raise-strong"
            role="status"
          >
            <div className="h-full w-1/2 rounded-full bg-primary motion-safe:animate-pulse" />
          </div>
        ) : null}
        {actionLabel && onAction ? (
          <button
            type="button"
            onClick={onAction}
            className="mt-6 rounded-xl border border-accent-rim bg-accent-soft px-4 py-2 text-sm font-bold text-accent transition hover:bg-accent/25 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
          >
            {actionLabel}
          </button>
        ) : null}
      </section>
    </main>
  )
}
