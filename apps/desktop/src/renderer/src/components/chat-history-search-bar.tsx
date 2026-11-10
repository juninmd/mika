import type { RefObject } from 'react'
import { useI18n } from '../hooks/useI18n'
import { CloseIcon } from './Icons'

type HistorySearchBarProps = {
  inputRef: RefObject<HTMLInputElement | null>
  value: string
  onChange: (value: string) => void
}

export function HistorySearchBar({ inputRef, value, onChange }: HistorySearchBarProps) {
  const { t } = useI18n()
  return (
    <div className="border-b border-border-subtle px-4 py-3">
      <div className="flex items-center gap-2 rounded-xl border border-border-subtle bg-surface-raise px-3 py-1.5 transition hover:border-border-strong focus-within:border-accent-rim focus-within:bg-surface-raise-strong">
        <svg
          className="size-3.5 shrink-0 text-text-muted"
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
          aria-hidden="true"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            d="M21 21l-4.35-4.35m0 0A7.5 7.5 0 104.5 4.5a7.5 7.5 0 0012.15 12.15z"
          />
        </svg>
        <input
          ref={inputRef}
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={t('shell.history.search')}
          aria-label={t('shell.history.searchLabel')}
          className="min-w-0 flex-1 bg-transparent text-xs text-text-main placeholder:text-text-muted focus:outline-none"
        />
        {value && (
          <button
            type="button"
            onClick={() => onChange('')}
            aria-label={t('shell.history.clearSearch')}
            className="shrink-0 text-text-muted transition-colors hover:text-text-main focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded"
          >
            <CloseIcon className="h-3 w-3" aria-hidden="true" />
          </button>
        )}
      </div>
    </div>
  )
}
