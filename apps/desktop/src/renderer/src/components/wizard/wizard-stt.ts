import type { SttId } from './wizard-data'

/** Valores canônicos de `voice.asrProvider` (`config/schemas/voice.rs::stt_fields`) que o wizard
 *  escolhe. A escuta ligar ou não é do toggle de microfone do passo de privacidade — aqui o ON
 *  é só "QUEM escuta". */
export const STT_PROVIDER_IDS: readonly SttId[] = ['nemotron', 'electron', 'browser', 'groq-whisper', 'openai-whisper']

export type SttSelection = SttId | null

// `local-whisper` é alias legado de `electron` — as Configurações acendem o MESMO card
// (`sttCardId`); o restore do wizard precisa devolver o mesmo id que a etapa pré-seleciona.
const STT_ALIAS: Record<string, SttId> = { 'local-whisper': 'electron' }

/** null = nada salvo (instalação nova) ou provedor fora do leque do wizard: mantém o que há. */
export function readSavedStt(config: Record<string, unknown>): SttSelection {
  const provider = (config.voice as Record<string, unknown> | undefined)?.asrProvider
  if (typeof provider !== 'string') return null
  const known = STT_PROVIDER_IDS.find((id) => id === provider)
  if (known) return known
  return STT_ALIAS[provider] ?? null
}

export function sttSettingsToSave(selection: Exclude<SttSelection, null>): Array<[string, unknown]> {
  return [['voice.asrProvider', selection]]
}
