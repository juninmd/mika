import { KeyRound, Mic2, Server, Volume2, Zap } from 'lucide-react'
import { useMemo } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { PageHero, StatusPill } from './ds'
import { CredentialSection, type Props, resolveFields } from './ProviderCredentialFields'
import { INTEGRATION_FIELD_SPECS, LLM_FIELD_SPECS, STT_FIELD_SPECS, TTS_FIELD_SPECS } from './provider-credential-specs'
import { getByPath } from './settings-utils'

export function ProviderCredentialsPanel({ config, onSave }: Props) {
  const { t } = useI18n()

  const llmFields = useMemo(() => resolveFields(LLM_FIELD_SPECS, t), [t])
  const ttsFields = useMemo(() => resolveFields(TTS_FIELD_SPECS, t), [t])
  const sttFields = useMemo(() => resolveFields(STT_FIELD_SPECS, t), [t])
  const integrationFields = useMemo(() => resolveFields(INTEGRATION_FIELD_SPECS, t), [t])

  const allSpecs = useMemo(
    () => [...LLM_FIELD_SPECS, ...TTS_FIELD_SPECS, ...STT_FIELD_SPECS, ...INTEGRATION_FIELD_SPECS],
    [],
  )
  const configuredCount = useMemo(() => {
    return allSpecs.filter((spec) => {
      const val = getByPath(config, spec.configPath)
      return typeof val === 'string' && val.trim().length > 0
    }).length
  }, [allSpecs, config])

  return (
    <div className="space-y-6">
      <PageHero
        icon={KeyRound}
        title={t('settings.providerCredentials.header.title')}
        description={t('settings.providerCredentials.header.description')}
        pills={
          <>
            <StatusPill
              variant={configuredCount > 0 ? 'ok' : 'pending'}
              label={t('settings.credentials.configuredCount')
                .replace('{{count}}', String(configuredCount))
                .replace('{{total}}', String(allSpecs.length))}
              dot
            />
            <StatusPill variant="neutral" label={t('settings.credentials.localOnly')} />
          </>
        }
        note={t('settings.providerCredentials.footer')}
      />

      <CredentialSection
        title={t('settings.providerCredentials.llm.title')}
        icon={Zap}
        description={t('settings.providerCredentials.llm.description')}
        header={t('settings.providerCredentials.llm.header')}
        fields={llmFields}
        config={config}
        onSave={onSave}
        t={t}
      />

      <CredentialSection
        title={t('settings.providerCredentials.tts.title')}
        icon={Volume2}
        description={t('settings.providerCredentials.tts.description')}
        header={t('settings.providerCredentials.tts.header')}
        fields={ttsFields}
        config={config}
        onSave={onSave}
        t={t}
      />

      <CredentialSection
        title={t('settings.providerCredentials.stt.title')}
        icon={Mic2}
        description={t('settings.providerCredentials.stt.description')}
        header={t('settings.providerCredentials.stt.header')}
        fields={sttFields}
        config={config}
        onSave={onSave}
        t={t}
      />

      <CredentialSection
        title={t('settings.providerCredentials.integrations.title')}
        icon={Server}
        description={t('settings.providerCredentials.integrations.description')}
        header={t('settings.providerCredentials.integrations.header')}
        fields={integrationFields}
        config={config}
        onSave={onSave}
        t={t}
      />
    </div>
  )
}
