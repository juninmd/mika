import type { LucideIcon } from 'lucide-react'
import { AuroraField, GlassSection, SectionLabel } from './ds'
import { CredentialCardActions } from './ProviderCredentialActions'
import type { CredentialField, CredentialFieldSpec, TKey } from './provider-credential-specs'
import { getByPath } from './settings-utils'

export type Props = {
  config: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
}

export function resolveFields(specs: CredentialFieldSpec[], t: (key: TKey) => string): CredentialField[] {
  return specs.map((spec) => ({
    label: t(spec.labelKey),
    configPath: spec.configPath,
    placeholder: spec.placeholderKey ? t(spec.placeholderKey) : (spec.placeholder ?? ''),
    type: spec.type,
    accentHue: spec.accentHue,
    description: spec.descriptionKey ? t(spec.descriptionKey) : undefined,
    expectedPrefix: spec.expectedPrefix,
    test: spec.test,
    keyUrl: spec.keyUrl,
  }))
}

function keyFormatWarning(field: CredentialField, config: Props['config'], t: (key: TKey) => string): string | null {
  if (!field.expectedPrefix) return null
  const value = getByPath(config, field.configPath)
  if (typeof value !== 'string' || value.length === 0) return null
  if (value.startsWith(field.expectedPrefix)) return null
  return t('settings.providerCredentials.keyFormatWarning')
    .replace('{{label}}', field.label)
    .replace('{{prefix}}', field.expectedPrefix)
}

function isFilled(config: Props['config'], path: string): boolean {
  const value = getByPath(config, path)
  return typeof value === 'string' && value.trim().length > 0
}

export function FieldGrid({
  fields,
  config,
  onSave,
  t,
}: {
  fields: CredentialField[]
  config: Props['config']
  onSave: Props['onSave']
  t: (key: TKey) => string
}) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      {fields.map((field) => {
        const warning = keyFormatWarning(field, config, t)
        return (
          <div
            key={field.configPath}
            data-configured={isFilled(config, field.configPath) || undefined}
            className="space-y-2 rounded-xl border border-border-subtle bg-glass p-3 transition-colors hover:border-border-strong data-configured:border-ok-rim/60"
          >
            <AuroraField
              label={field.label}
              configPath={field.configPath}
              config={config}
              onSave={onSave}
              type={field.type}
              placeholder={field.placeholder}
              accentHue={field.accentHue}
              description={field.description}
            />
            {warning && (
              <p role="alert" className="text-caption leading-snug text-warn/90">
                {warning}
              </p>
            )}
            <CredentialCardActions
              field={field}
              config={config}
              filled={isFilled(config, field.configPath)}
              onSave={onSave}
              t={t}
            />
          </div>
        )
      })}
    </div>
  )
}

export function CredentialSection({
  title,
  icon: Icon,
  description,
  header,
  fields,
  config,
  onSave,
  t,
}: {
  title: string
  icon: LucideIcon
  description: string
  header: string
  fields: CredentialField[]
  config: Props['config']
  onSave: Props['onSave']
  t: (key: TKey) => string
}) {
  const filled = fields.filter((f) => isFilled(config, f.configPath)).length
  return (
    <GlassSection title={title} icon={Icon} badge={`${filled} / ${fields.length}`} description={description}>
      <SectionLabel className="mb-3">{header}</SectionLabel>
      <FieldGrid fields={fields} config={config} onSave={onSave} t={t} />
    </GlassSection>
  )
}
