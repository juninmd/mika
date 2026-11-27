import { MAX_UTTERANCE_MS } from './speech-transcribe'

/** Teto de segurança do fim de turno inteligente: mesmo com a frase "incompleta", a pausa não pode
 * passar disto (3 s, ou o hangover do VAD se ele já for maior). */
export const SMART_TURN_MAX_SILENCE_MS = 3_000
/** Timeout de UMA classificação. Erro/timeout cai no VAD e encerra — nunca prende o usuário. */
export const SMART_TURN_CHECK_TIMEOUT_MS = 1_500
/** Últimos 8 s de PCM16 mono 16 kHz: 8 × 16.000 × 2 bytes. */
export const SMART_TURN_WINDOW_BYTES = 256_000

export type EndOfTurnCheck =
  | { status: 'idle' }
  | { status: 'pending'; voiceAt: number }
  | { status: 'complete'; voiceAt: number; probability: number | null; latencyMs: number }
  | { status: 'incomplete'; voiceAt: number; probability: number | null; latencyMs: number }
  | { status: 'error'; voiceAt: number }

export type EndOfTurnInput = {
  silentForMs: number
  elapsedMs: number
  hangoverMs: number
  maxSilenceMs: number
  endpointingEnabled: boolean
  smartTurnEnabled: boolean
  lastVoiceAt: number
  check: EndOfTurnCheck
}

export type EndOfTurnAction = 'finish' | 'check' | 'wait'

/** Últimos 8 s de PCM16 (cópia: o buffer da utterance continua crescendo). */
export function lastSmartTurnWindow(pcm: Uint8Array): Uint8Array {
  return pcm.length <= SMART_TURN_WINDOW_BYTES ? pcm : pcm.slice(pcm.length - SMART_TURN_WINDOW_BYTES)
}

/** Uma checagem pertence ao silêncio que começou no `lastVoiceAt` atual. Voz depois dela torna o
 * resultado obsoleto — nunca encerra uma frase com base numa pausa que já passou. */
function activeCheck(check: EndOfTurnCheck, lastVoiceAt: number): EndOfTurnCheck {
  return check.status !== 'idle' && check.voiceAt === lastVoiceAt ? check : { status: 'idle' }
}

/**
 * Máquina de decisão do fim de turno, sem timers nem rede. `finish` encerra a utterance; `check`
 * pede UMA classificação Smart Turn; `wait` mantém a escuta.
 *
 * Sem Smart Turn (ou sem PCM capturado, que o chamador representa desligando o sinalizador), o
 * caminho é EXATAMENTE o antigo: teto de 45 s e, com endpointing ligado, o hangover do VAD.
 */
export function decideEndOfTurn(input: EndOfTurnInput): EndOfTurnAction {
  if (input.elapsedMs >= MAX_UTTERANCE_MS) return 'finish'
  if (!input.endpointingEnabled) return 'wait'
  if (!input.smartTurnEnabled) return input.silentForMs >= input.hangoverMs ? 'finish' : 'wait'

  // Piso de segurança primeiro: uma classificação pendente ou "incompleta" não pode esticar a pausa
  // além do teto.
  if (input.silentForMs >= input.maxSilenceMs) return 'finish'
  if (input.silentForMs < input.hangoverMs) return 'wait'

  const check = activeCheck(input.check, input.lastVoiceAt)
  if (check.status === 'complete') return 'finish'
  if (check.status === 'error') return 'finish'
  if (check.status === 'pending' || check.status === 'incomplete') return 'wait'
  return 'check'
}
