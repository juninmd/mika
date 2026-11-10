import {
  AuroraField,
  BentoGrid,
  GlassCard,
  GlassSection,
  GlassSelect,
  InfoRow,
  SectionHeader,
  StatusPill,
} from '@mika/sdk-ui/settings'
import { Sparkles, Wand2 } from 'lucide-react'
import type { ComponentProps } from 'react'
import { AVATAR_MODELS, resolveLive2DModelPath } from '../../../renderer/src/components/live2d/model-path'
import { RelationshipSelector } from '../../../renderer/src/components/settings/RelationshipSelector'
import { useI18n } from '../../../renderer/src/hooks/useI18n'
import {
  APP_LOCALE_OPTIONS,
  backgroundModeOptions,
  type CharacterIdentityPanelProps,
  profileOptions,
  resolveIdentityConfig,
  responseStyleOptions,
} from './character-identity-config'

type GlassSelectProps = ComponentProps<typeof GlassSelect>

export function CharacterIdentityPanel(props: CharacterIdentityPanelProps) {
  const { t } = useI18n()
  const { enabled, name, modelName, modelPath, backgroundMode, relationshipStatus, affinityScore, cfg } =
    resolveIdentityConfig(props)

  const f = (
    configPath: string,
    label: string,
    opts?: {
      placeholder?: string
      type?: 'text' | 'number' | 'password' | 'url' | 'email' | 'textarea'
      rows?: number
    },
  ) => <AuroraField label={label} configPath={configPath} config={cfg} onSave={props.onSave} {...opts} />

  const sel = (configPath: string, label: string, options: GlassSelectProps['options']) => (
    <GlassSelect
      label={label}
      configPath={configPath}
      options={options}
      config={cfg}
      onSave={props.onSave as (p: string, v: string) => Promise<void>}
    />
  )

  const resolvedModelPath = resolveLive2DModelPath({ modelName, modelPath })
  const modelLabel =
    AVATAR_MODELS.find((m) => m.path === resolvedModelPath)?.label ?? t('settings.character.identityPanel.customModel')
  const isEnabled = enabled

  return (
    <GlassSection
      title={
        <div className="flex items-center gap-3">
          <span>{name || t('settings.character.identityPanel.defaultName')}</span>
          <StatusPill
            variant={isEnabled ? 'ok' : 'pending'}
            label={
              isEnabled
                ? t('settings.character.identityPanel.statusActive')
                : t('settings.character.identityPanel.statusOff')
            }
            dot
          />
        </div>
      }
      icon={Sparkles}
      description={t('settings.character.identityPanel.description')}
    >
      <BentoGrid cols="auto">
        <GlassCard span="full">
          <SectionHeader title={t('settings.character.identityPanel.basicInfo')} className="mb-3" />
          <div className="grid gap-4 sm:grid-cols-2">
            {f('character.name', t('settings.character.field.name.label'), { placeholder: 'Mika' })}
            {sel('app.locale', t('label.language'), APP_LOCALE_OPTIONS)}
            <div className="sm:col-span-2">
              {sel(
                'character.responseStyle',
                t('settings.character.field.responseStyle.label'),
                responseStyleOptions(t),
              )}
            </div>
          </div>
        </GlassCard>

        <GlassCard span="full">
          <RelationshipSelector value={relationshipStatus} affinityScore={affinityScore} onSave={props.onSave} />
        </GlassCard>

        <GlassCard span="full">
          <SectionHeader title={t('settings.character.identityPanel.coreTraits')} className="mb-3" />
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="sm:col-span-2">
              {sel(
                'character.personalityProfile',
                t('settings.character.field.personalityProfile.label'),
                profileOptions(t),
              )}
            </div>
            {f('character.personality', t('settings.character.field.personality.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.character.field.personality.placeholder'),
            })}
            {f('character.visualDescription', t('settings.character.field.visualDescription.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.character.field.visualDescription.placeholder'),
            })}
            {f('character.greetingStyle', t('settings.character.field.greetingStyle.label'), {
              placeholder: t('settings.character.field.greetingStyle.placeholder'),
            })}
            {/* Free-form and usually the longest text here: a half column left the other half empty. */}
            <div className="sm:col-span-2">
              {f('character.customInstructions', t('settings.character.identityPanel.customInstructionsLabel'), {
                type: 'textarea',
                rows: 3,
                placeholder: t('settings.character.identityPanel.customInstructionsPlaceholder'),
              })}
            </div>
          </div>
        </GlassCard>

        <GlassCard span="full">
          <SectionHeader title={t('settings.character.identityPanel.likesDislikes')} className="mb-3" />
          <div className="grid gap-4 sm:grid-cols-2">
            {f('character.hobbies', t('settings.character.field.hobbies.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.character.field.hobbies.placeholder'),
            })}
            {f('character.dislikes', t('settings.character.field.dislikes.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.character.field.dislikes.placeholder'),
            })}
            {f('character.nicknamesForUser', t('settings.character.field.nicknamesForUser.label'), {
              placeholder: t('settings.character.field.nicknamesForUser.placeholder'),
            })}
            {f('character.secretGoals', t('settings.character.field.secretGoals.label'), {
              type: 'textarea',
              rows: 2,
              placeholder: t('settings.character.field.secretGoals.placeholder'),
            })}
          </div>
        </GlassCard>

        <GlassCard span="full">
          <SectionHeader title={t('settings.character.identityPanel.live2dAppearance')} className="mb-3" />
          <div className="grid gap-4">
            <div className="space-y-2 rounded-xl border border-border-subtle bg-surface-raise p-3">
              <InfoRow label={t('settings.character.field.modelName.label')} value={modelLabel} />
              <InfoRow label={t('settings.character.identityPanel.modelPathShortLabel')} value={modelPath} mono />
              <p className="flex items-center gap-1.5 text-caption leading-snug text-text-muted">
                <Wand2 size={12} aria-hidden="true" />
                {t('settings.character.identityPanel.switchModelHint')}
              </p>
            </div>
            {sel(
              'character.backgroundMode',
              t('settings.character.field.backgroundMode.label'),
              backgroundModeOptions(t),
            )}
            {backgroundMode === 'solid' &&
              f('character.backgroundSolidColor', t('settings.character.identityPanel.bgColorLabel'), {
                placeholder: '#050914',
              })}
          </div>
        </GlassCard>
      </BentoGrid>
    </GlassSection>
  )
}
