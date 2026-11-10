import { describe, expect, it } from 'vitest'
import { type AssetTask, buildInitialTasks } from '../wizard-assets'

const copy = {
  back: 'Voltar',
  preparing: 'Preparando',
  readyEyebrow: 'Pronta',
  readyTitle: 'Tudo pronto',
  preparingEyebrow: 'Preparando',
  preparingTitle: 'Baixando o que ela precisa',
  statusRunning: 'Baixando',
  statusDone: 'Ok',
  statusSkipped: 'Fora do escopo',
  statusError: 'Erro',
  statusPending: 'Pendente',
  errorHint: 'Tente de novo',
  retry: 'Tentar de novo',
  downloadFailed: 'Falhou',
  vadLabel: 'VAD',
  vadDetail: 'detector de fala',
  sttLabel: 'STT',
  sttDetail: 'modelo de escuta',
  ttsLabel: 'TTS',
  ttsSupertonic: 'voz local',
  ttsCloud: 'voz nuvem',
  llmLabel: 'LLM',
  embeddingLabel: 'Embedding',
  embeddingDetail: 'memória',
} as const

// Copia narrowed para o que buildInitialTasks lê.
type AssetsCopy = Parameters<typeof buildInitialTasks>[0]

function taskIds(tasks: AssetTask[]): string[] {
  return tasks.map((t) => t.id)
}

describe('buildInitialTasks — escuta condicionada', () => {
  // Quem não liga a escuta NÃO paga pelo conjunto VAD+STT: modelos baixados e nunca usados
  // gastam banda/espaço à toa (mesma regra do Ollama pull, que só entra se o cérebro é dele).
  it('sem escuta: nem vad nem STT (nemotron) entram na fila', () => {
    const tasks = buildInitialTasks(
      copy as unknown as AssetsCopy,
      false,
      true,
      'qwen3.5:4b',
      'supertonic',
      false,
      'nemotron',
    )
    expect(taskIds(tasks)).toEqual(['tts', 'local-embedding'])
  })

  it('com escuta no nemotron: vad + nemotron entram', () => {
    const tasks = buildInitialTasks(
      copy as unknown as AssetsCopy,
      false,
      true,
      'qwen3.5:4b',
      'supertonic',
      true,
      'nemotron',
    )
    expect(taskIds(tasks)).toEqual(['vad', 'nemotron', 'tts', 'local-embedding'])
  })

  it('com escuta em provedor de nuvem (groq-whisper): só vad — o modelo de spoken não é local', () => {
    const tasks = buildInitialTasks(
      copy as unknown as AssetsCopy,
      false,
      true,
      'qwen3.5:4b',
      'supertonic',
      true,
      'groq-whisper',
    )
    expect(taskIds(tasks)).toEqual(['vad', 'tts', 'local-embedding'])
  })

  it('voz off: tarefa de TTS sai da fila', () => {
    const tasks = buildInitialTasks(copy as unknown as AssetsCopy, false, true, 'qwen3.5:4b', null, true, 'nemotron')
    expect(taskIds(tasks)).toEqual(['vad', 'nemotron', 'local-embedding'])
  })
})
