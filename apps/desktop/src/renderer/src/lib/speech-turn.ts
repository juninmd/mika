import { isImplausibleTranscript } from '@mika/sdk/utils/transcript-plausibility'
import { MIN_TRANSCRIPT_CHARS } from '../hooks/shell-commands'

// O filtro de alucinação mora no SDK: o gravador de barge-in, no main, recebe as mesmas frases de
// estoque do Whisper e precisa da mesma régua. Reexportado para o hook e para a suíte continuarem
// falando com este módulo.
export { hasDegenerateRepetition, isImplausibleTranscript } from '@mika/sdk/utils/transcript-plausibility'

/** Fala guardada mais velha que isto é ruído de contexto: o assunto já passou. */
export const HELD_SPEECH_TTL_MS = 45_000

type SpokenDecision = 'too-short' | 'implausible' | 'hold' | 'send'

/** O que fazer com uma fala transcrita: `hold` guarda em vez de descartar quando um turno já roda. */
export function decideSpokenTurn(input: { text: string; durationMs?: number; busy: boolean }): SpokenDecision {
  if (input.text.length < MIN_TRANSCRIPT_CHARS) return 'too-short'
  if (isImplausibleTranscript(input.text, input.durationMs)) return 'implausible'
  return input.busy ? 'hold' : 'send'
}

/** Responder a algo de um minuto atrás soa mais quebrado do que não responder. */
export function isHeldSpeechStale(heldAt: number, now: number): boolean {
  return now - heldAt > HELD_SPEECH_TTL_MS
}
