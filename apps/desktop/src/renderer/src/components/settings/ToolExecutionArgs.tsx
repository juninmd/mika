import type { ToolParameterDetail } from '@mika/sdk/contracts'
import { AuroraField, GlassToggle } from '@mika/sdk-ui/settings'
import { useI18n } from '../../hooks/useI18n'
import { isSensitiveArgName } from '../../lib/sensitive-tool-arg'
import { type ArgMap, argBadge, inferInputType } from './tool-execution-args'

function ArgField({
  param,
  value,
  onChange,
}: {
  param: ToolParameterDetail
  value: string
  onChange: (name: string, value: string) => void
}) {
  const { t } = useI18n()
  const isSensitive = isSensitiveArgName(param.name)
  const baseInputType = inferInputType(param.type)
  const inputType = isSensitive && baseInputType === 'text' ? 'password' : baseInputType
  const badge = argBadge(param, t)

  if (inputType === 'checkbox') {
    return (
      <GlassToggle
        label={param.name}
        description={param.description}
        badge={badge}
        value={value === 'true'}
        onChange={(next) => onChange(param.name, next ? 'true' : 'false')}
      />
    )
  }

  return (
    <AuroraField
      label={param.name}
      description={param.description}
      badge={badge}
      type={inputType === 'textarea' ? 'textarea' : (inputType as 'text' | 'number' | 'password')}
      rows={3}
      value={value}
      onChange={(next) => onChange(param.name, next)}
      placeholder={
        inputType === 'textarea'
          ? param.description || `{ "key": "value" }`
          : isSensitive
            ? '••••••'
            : param.description || param.name
      }
    />
  )
}

export function ArgsForm({
  params,
  args,
  onChange,
}: {
  params: ToolParameterDetail[]
  args: ArgMap
  onChange: (name: string, value: string) => void
}) {
  if (params.length === 0) return null

  return (
    <div className="grid gap-2">
      {params.map((param) => (
        <ArgField key={param.name} param={param} value={args[param.name] ?? ''} onChange={onChange} />
      ))}
    </div>
  )
}

export function ResultBlock({ data }: { data: unknown }) {
  const formatted = (() => {
    if (data === null || data === undefined) return 'void'
    if (typeof data === 'string') return data
    try {
      return JSON.stringify(data, null, 2)
    } catch {
      return String(data)
    }
  })()

  return (
    <pre className="max-h-48 overflow-auto rounded-lg border border-border-subtle bg-surface-raise/60 p-3 text-caption font-mono leading-relaxed text-text-main [scrollbar-width:thin] [scrollbar-color:var(--primary)_transparent]">
      {formatted}
    </pre>
  )
}
