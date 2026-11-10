import { useI18n } from '../hooks/useI18n'

export function AlertTriangleIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
    >
      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3.05h16.94a2 2 0 0 0 1.71-3.05l-8.47-14.14a2 2 0 0 0-3.42 0z" />
      <line x1="12" y1="9" x2="12" y2="13" />
      <line x1="12" y1="17" x2="12.01" y2="17" />
    </svg>
  )
}

/** Raw arguments are for the curious: collapsed by default so the decision reads in one glance. */
export function ToolApprovalArgs({ argEntries }: { argEntries: string[] }) {
  const { t } = useI18n()
  return (
    <details className="group mb-4 rounded-lg bg-surface-raise border border-border-subtle">
      <summary className="flex cursor-pointer list-none items-center justify-between gap-2 rounded-lg px-3 py-2 text-xs font-medium text-text-muted hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus [&::-webkit-details-marker]:hidden">
        <span>{t('shell.approval.details')}</span>
        <span aria-hidden="true" className="transition-transform group-open:rotate-90">
          ›
        </span>
      </summary>
      <div className="px-3 pb-3">
        <p className="text-xs text-text-muted mb-2 font-mono font-medium">{t('shell.approval.args')}</p>
        <div className="text-caption text-text-main font-mono space-y-1 max-h-24 overflow-y-auto">
          {argEntries.map((entry) => (
            <div key={entry} className="truncate" title={entry}>
              {entry}
            </div>
          ))}
        </div>
      </div>
    </details>
  )
}

/** Remaining time as a ring that drains; the numeric `role="timer"` line stays the accessible source. */
export function ApprovalCountdownRing({ remaining, total }: { remaining: number; total: number }) {
  const radius = 9
  const circumference = 2 * Math.PI * radius
  const fraction = total > 0 ? Math.min(1, Math.max(0, remaining / total)) : 0
  return (
    <svg width="22" height="22" viewBox="0 0 22 22" aria-hidden="true" focusable="false" className="shrink-0">
      <circle cx="11" cy="11" r={radius} fill="none" strokeWidth="2.5" className="stroke-border-strong" />
      <circle
        cx="11"
        cy="11"
        r={radius}
        fill="none"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - fraction)}
        transform="rotate(-90 11 11)"
        className="stroke-warn transition-[stroke-dashoffset] duration-1000 ease-linear"
      />
    </svg>
  )
}
