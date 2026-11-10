import { getLLMProviderEntry } from '@mika/sdk/llm/llm-catalog-data'
import { getElectronApi } from '../../lib/shell-api'
import type { WizardCopy } from './wizard-copy'
import type { SttSelection } from './wizard-stt'
import type { TtsSelection } from './wizard-tts'

export interface AssetTask {
  id: 'vad' | 'nemotron' | 'local-embedding' | 'tts' | 'ollama-llm'
  label: string
  detail: string
  status: 'pending' | 'running' | 'done' | 'skipped' | 'error'
  errorMessage?: string
}

export function buildInitialTasks(
  copy: WizardCopy['assets'],
  isOllama: boolean,
  llmConfigured: boolean,
  llmModel: string,
  tts: TtsSelection,
  listening: boolean,
  stt: SttSelection,
): AssetTask[] {
  const ttsDetail = tts === 'supertonic' ? copy.ttsSupertonic : copy.ttsCloud

  const list: AssetTask[] = []

  // Locks que ninguém usar são banda/espaço à toa: quem desativou a escuta (passo de privacidade)
  // não baixa VAD nem STT; e entre provedores de escuta, só o nemotron tem modelo local próprio.
  if (listening) {
    list.push({
      id: 'vad',
      label: copy.vadLabel,
      detail: copy.vadDetail,
      status: 'pending',
    })
    if (stt === 'nemotron') {
      list.push({
        id: 'nemotron',
        label: copy.sttLabel,
        detail: copy.sttDetail,
        status: 'pending',
      })
    }
  }

  if (tts) {
    list.push({
      id: 'tts',
      label: copy.ttsLabel,
      detail: ttsDetail,
      status: 'pending',
    })
  }

  list.push({
    id: 'local-embedding',
    label: copy.embeddingLabel,
    detail: copy.embeddingDetail,
    status: 'pending',
  })

  if (isOllama && llmConfigured) {
    list.push({
      id: 'ollama-llm',
      label: copy.llmLabel,
      detail: `Ollama pull: ${llmModel || getLLMProviderEntry('ollama').defaultModel}`,
      status: 'pending',
    })
  }

  return list
}

export async function executeAssetTask(
  copy: WizardCopy['assets'],
  task: AssetTask,
  tts: TtsSelection,
  llmProvider: string,
  llmModel: string,
): Promise<{ status: 'done' | 'skipped' | 'error'; errorMessage?: string }> {
  try {
    const res = await getElectronApi().runtime.prepareWizardAsset({
      assetId: task.id,
      ttsProvider: tts ?? undefined,
      ttsVoice: tts === 'supertonic' ? 'F3' : undefined,
      llmProvider,
      llmModel,
    })
    return {
      status: res.skipped ? 'skipped' : res.success ? 'done' : 'error',
      errorMessage: res.success ? undefined : res.message || copy.downloadFailed,
    }
  } catch (err) {
    return {
      status: 'error',
      errorMessage: err instanceof Error ? err.message : String(err),
    }
  }
}
