interface SectionDividerProps {
  label?: string
}

export function SectionDivider({ label }: SectionDividerProps) {
  if (!label) {
    return <div className="border-t border-border-subtle my-4" />
  }

  return (
    <div className="relative my-4 flex items-center">
      <div className="flex-1 border-t border-border-subtle" />
      <div className="px-3">
        <span className="text-caption font-semibold uppercase tracking-[0.12em] text-text-muted">{label}</span>
      </div>
      <div className="flex-1 border-t border-border-subtle" />
    </div>
  )
}
