import type { ReactNode } from 'react'

interface InfoRowProps {
  label: string
  value: string | ReactNode
  mono?: boolean
}

export function InfoRow({ label, value, mono = false }: InfoRowProps) {
  return (
    <div className="flex flex-col gap-1 sm:flex-row sm:items-center sm:justify-between">
      <span className="text-caption font-semibold uppercase tracking-[0.08em] text-text-muted">{label}</span>
      <span className={`text-[12px] text-text-main ${mono ? 'font-mono' : ''}`}>{value}</span>
    </div>
  )
}
