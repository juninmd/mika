import { Check } from 'lucide-react'
import type { ReactNode } from 'react'

export function StepHeader({ step, title, description }: { step: string; title: string; description: string }) {
  return (
    <header className="mb-4">
      <div className="flex items-center gap-2">
        <span className="text-micro font-black uppercase tracking-[0.18em] text-accent">{step}</span>
      </div>
      <h2 className="mt-1.5 text-xl font-black tracking-tight text-text-main md:text-2xl">{title}</h2>
      <p className="mt-1 text-xs leading-relaxed text-text-muted">{description}</p>
    </header>
  )
}

/** Mesma linguagem visual de `GlassChoiceCards` (marca de seleção, filete superior, hover-lift sem motion reduzido). */
export function OptionCard({
  description,
  disabled,
  icon,
  onClick,
  selected,
  title,
}: {
  description: string
  disabled?: boolean
  /** Lucide glyph that says what the choice is (local, cloud, voice off…) before the text is read. */
  icon?: ReactNode
  onClick: () => void
  selected: boolean
  title: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      aria-pressed={selected}
      className={`group relative flex w-full items-start gap-3 overflow-hidden rounded-2xl border p-3 text-left transition-[transform,background-color,border-color,box-shadow] duration-150 ease-out active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50 motion-reduce:transition-none sm:p-3.5 ${
        selected
          ? 'border-accent-rim bg-accent-soft shadow-[0_0_18px_var(--accent-glow)]'
          : 'border-border-subtle bg-surface-raise enabled:hover:-translate-y-0.5 enabled:hover:border-border-strong enabled:hover:bg-surface-raise-strong motion-reduce:enabled:hover:translate-y-0'
      }`}
    >
      {selected && (
        <span
          aria-hidden="true"
          className="absolute inset-x-0 top-0 h-0.5 bg-accent shadow-[0_0_10px_var(--accent-glow)]"
        />
      )}
      {icon && (
        <span
          aria-hidden="true"
          className={`flex size-8 shrink-0 items-center justify-center rounded-xl border transition-colors duration-150 ${selected ? 'border-accent-rim bg-accent/15 text-accent-text' : 'border-border-subtle bg-surface-raise-strong text-text-muted'}`}
        >
          {icon}
        </span>
      )}
      <span className="min-w-0 flex-1">
        <span
          className={`block text-xs font-black uppercase tracking-[0.12em] ${selected ? 'text-accent-text' : 'text-text-main'}`}
        >
          {title}
        </span>
        <span className="mt-1 block text-caption leading-relaxed text-text-muted">{description}</span>
      </span>
      <span
        aria-hidden="true"
        className={`mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border transition-[transform,background-color,border-color,color] duration-150 ${
          selected ? 'scale-100 border-accent bg-accent text-bg-deep' : 'scale-90 border-border-subtle text-transparent'
        }`}
      >
        <Check size={11} strokeWidth={3} />
      </span>
    </button>
  )
}

export function ErrorBanner({ message }: { message: string }) {
  return (
    <div role="alert" className="mt-4 rounded-xl border border-danger-rim bg-danger-soft px-3 py-2 text-xs text-danger">
      {message}
    </div>
  )
}

export function PrimaryButton({
  children,
  disabled,
  onClick,
}: {
  children: ReactNode
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="inline-flex items-center justify-center rounded-xl border border-accent-rim bg-accent-soft px-5 py-2.5 text-xs font-black uppercase tracking-[0.12em] text-accent-text shadow-[0_0_18px_var(--accent-glow)] transition-[background-color,transform] duration-150 hover:bg-accent/25 active:scale-[0.98] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
    >
      {children}
    </button>
  )
}

export function SecondaryButton({
  children,
  disabled,
  onClick,
}: {
  children: ReactNode
  disabled?: boolean
  onClick: () => void
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className="inline-flex items-center justify-center rounded-xl border border-border-subtle bg-surface-raise px-5 py-2.5 text-xs font-black uppercase tracking-[0.12em] text-text-main transition-[background-color,border-color,transform] duration-150 hover:border-border-strong active:scale-[0.98] hover:bg-surface-raise-strong focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus disabled:cursor-not-allowed disabled:opacity-50"
    >
      {children}
    </button>
  )
}
