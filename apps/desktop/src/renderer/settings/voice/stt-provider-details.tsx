import type { TranslationDict } from '@mika/sdk/i18n'
import { GlassSelect } from '@mika/sdk-ui/settings'
import type { ReactNode } from 'react'
import { ModelDownloadCard } from '../model-download-card'

type T = (key: keyof TranslationDict) => string
type Option = { label: string; value: string; description?: string; icon?: ReactNode }

/** Mesmas listas de `stt_cloud::CloudSttProvider::models` (Rust); o rótulo é o id do modelo. */
const CLOUD_STT_MODELS: Record<string, { path: string; models: string[] }> = {
  'groq-whisper': { path: 'voice.groqSttModel', models: ['whisper-large-v3-turbo', 'whisper-large-v3'] },
  'openai-whisper': {
    path: 'voice.openaiSttModel',
    models: ['gpt-transcribe', 'gpt-4o-mini-transcribe', 'gpt-4o-transcribe', 'whisper-1'],
  },
}

type SttDetailsProps = {
  t: T
  providerId: string
  cfg: Record<string, unknown>
  onSave: (path: string, value: unknown) => Promise<void>
  selectAsrProvider: (value: string) => Promise<void>
  whisperModelOptions: Option[]
}

function BrowserWarning({ t, selectAsrProvider }: Pick<SttDetailsProps, 't' | 'selectAsrProvider'>) {
  return (
    <div className="flex flex-wrap items-center gap-2 rounded-lg border border-warn-rim bg-warn-soft px-3 py-2 text-caption text-warn/90">
      <p className="min-w-[16rem] flex-1">
        {t('settings.voiceStt.panel.browserWarningPrefix')}{' '}
        <span className="font-bold">{t('settings.voiceStt.panel.browserWarningLabel')}</span>{' '}
        {t('settings.voiceStt.panel.browserWarningMiddle')} <span className="font-mono">network</span>.
      </p>
      <button
        type="button"
        onClick={() => void selectAsrProvider('local-whisper')}
        className="shrink-0 rounded-md border border-warn/40 bg-warn-soft px-2.5 py-1 font-bold text-warn transition-colors hover:bg-warn/25"
      >
        {t('settings.voiceStt.panel.useLocalWhisper')}
      </button>
    </div>
  )
}

export function SttProviderDetails({
  t,
  providerId,
  cfg,
  onSave,
  selectAsrProvider,
  whisperModelOptions,
}: SttDetailsProps) {
  if (providerId === 'nemotron') {
    return (
      <ModelDownloadCard
        assetId="nemotron"
        labels={{
          hint: 'settings.voiceStt.panel.nemotronHint',
          ready: 'settings.voiceStt.panel.nemotronReady',
          download: 'settings.voiceStt.panel.nemotronDownload',
          downloading: 'settings.voiceStt.panel.nemotronDownloading',
        }}
        t={t}
      />
    )
  }
  if (providerId === 'browser') return <BrowserWarning t={t} selectAsrProvider={selectAsrProvider} />
  if (providerId === 'electron') {
    return (
      <>
        <GlassSelect
          label={t('settings.voiceStt.panel.whisperModel.label')}
          configPath="voice.whisperModel"
          options={whisperModelOptions}
          config={cfg}
          onSave={onSave}
        />
        <ModelDownloadCard
          assetId="whisper"
          labels={{
            hint: 'settings.voiceStt.panel.whisperDownloadHint',
            ready: 'settings.provider.model.ready',
            download: 'settings.provider.model.download',
            downloading: 'settings.provider.model.downloading',
          }}
          t={t}
        />
      </>
    )
  }
  const cloud = CLOUD_STT_MODELS[providerId]
  if (!cloud) return null
  return (
    <>
      <GlassSelect
        label={t('settings.voiceStt.field.cloudModel.label')}
        configPath={cloud.path}
        variant="dropdown"
        options={cloud.models.map((id) => ({ label: id, value: id }))}
        config={cfg}
        onSave={onSave}
      />
      <p className="text-caption text-text-muted">{t('settings.provider.apiKeyHint')}</p>
    </>
  )
}

// Only Silero has details (its model download); the activity threshold moved to the
// "Calibrar microfone" card, drawn on the live meter.
export function SileroVadDetails({ t }: { t: T }) {
  return (
    <ModelDownloadCard
      assetId="vad"
      labels={{
        hint: 'settings.voiceStt.vadEngine.silero.downloadHint',
        ready: 'settings.provider.model.ready',
        download: 'settings.provider.model.download',
        downloading: 'settings.provider.model.downloading',
      }}
      t={t}
    />
  )
}
