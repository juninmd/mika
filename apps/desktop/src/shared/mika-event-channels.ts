import { DESKTOP_BRIDGED_EVENT_CHANNELS, DESKTOP_EVENT_CHANNELS } from '@mika/sdk/contracts'

export const MIKA_EVENT_CHANNELS = {
  ttsAudio: DESKTOP_EVENT_CHANNELS.TtsAudio,
  chatStreamChunk: DESKTOP_EVENT_CHANNELS.ChatStreamChunk,
  chatAssistantMessage: DESKTOP_EVENT_CHANNELS.ChatAssistantMessage,
  avatarExpression: DESKTOP_EVENT_CHANNELS.AvatarExpression,
  chatToolCall: DESKTOP_EVENT_CHANNELS.ChatToolCall,
  chatToolResult: DESKTOP_EVENT_CHANNELS.ChatToolResult,
  visionScreenObservation: DESKTOP_EVENT_CHANNELS.VisionScreenObservation,
  chatThinkingStart: DESKTOP_EVENT_CHANNELS.ChatThinkingStart,
  chatThinkingStop: DESKTOP_EVENT_CHANNELS.ChatThinkingStop,
  chatError: DESKTOP_BRIDGED_EVENT_CHANNELS.ChatError,
  moodChanged: DESKTOP_BRIDGED_EVENT_CHANNELS.MoodChanged,
  brainProactiveThought: DESKTOP_BRIDGED_EVENT_CHANNELS.BrainProactiveThought,
  listeningStarted: DESKTOP_BRIDGED_EVENT_CHANNELS.ListeningStarted,
  listeningStopped: DESKTOP_BRIDGED_EVENT_CHANNELS.ListeningStopped,
  forceStopTts: DESKTOP_BRIDGED_EVENT_CHANNELS.ForceStopTts,
  interruption: DESKTOP_BRIDGED_EVENT_CHANNELS.Interruption,
  powerBattery: DESKTOP_BRIDGED_EVENT_CHANNELS.PowerBattery,
  powerAc: DESKTOP_BRIDGED_EVENT_CHANNELS.PowerAc,
  notification: DESKTOP_BRIDGED_EVENT_CHANNELS.Notification,
  personCatalogUpdated: DESKTOP_BRIDGED_EVENT_CHANNELS.PersonCatalogUpdated,
  personCatalogDeleted: DESKTOP_BRIDGED_EVENT_CHANNELS.PersonCatalogDeleted,
  activity: DESKTOP_BRIDGED_EVENT_CHANNELS.Activity,
  sensesPaused: DESKTOP_BRIDGED_EVENT_CHANNELS.SensesPaused,
} as const

export const MIKA_BRIDGED_EVENT_CHANNELS = {
  'chat:stream-chunk': MIKA_EVENT_CHANNELS.chatStreamChunk,
  'chat:assistant-message': MIKA_EVENT_CHANNELS.chatAssistantMessage,
  'avatar:expression': MIKA_EVENT_CHANNELS.avatarExpression,
  'chat:tool-call': MIKA_EVENT_CHANNELS.chatToolCall,
  'chat:tool-result': MIKA_EVENT_CHANNELS.chatToolResult,
  'voice:tts-audio-ready': MIKA_EVENT_CHANNELS.ttsAudio,
  'vision:screen-observation': MIKA_EVENT_CHANNELS.visionScreenObservation,
  'chat:thinking-start': MIKA_EVENT_CHANNELS.chatThinkingStart,
  'chat:thinking-stop': MIKA_EVENT_CHANNELS.chatThinkingStop,
  'chat:error': MIKA_EVENT_CHANNELS.chatError,
  'mood:changed': MIKA_EVENT_CHANNELS.moodChanged,
  'brain:proactive-thought': MIKA_EVENT_CHANNELS.brainProactiveThought,

  'voice:listening-started': MIKA_EVENT_CHANNELS.listeningStarted,
  'voice:listening-stopped': MIKA_EVENT_CHANNELS.listeningStopped,

  'voice:force-stop-tts': MIKA_EVENT_CHANNELS.forceStopTts,
  // defects.md "barge-in", item 1: o barge-in de voz já publicava isto no Rust; faltava chegar ao
  // renderer para `useShellVoice.ts` descartar seu próprio envio duplicado da mesma fala.
  'voice:interruption': MIKA_EVENT_CHANNELS.interruption,

  'system:power-battery': MIKA_EVENT_CHANNELS.powerBattery,
  'system:power-ac': MIKA_EVENT_CHANNELS.powerAc,

  notification: MIKA_EVENT_CHANNELS.notification,

  'person-catalog:updated': MIKA_EVENT_CHANNELS.personCatalogUpdated,
  // Uma menção nova altera o registro tanto quanto uma edição: o painel aberto precisa recarregar.
  // Sem esta linha o evento era emitido e ninguém escutava — `check:events` acusou.
  'person-catalog:mention-added': MIKA_EVENT_CHANNELS.personCatalogUpdated,
  'person-catalog:deleted': MIKA_EVENT_CHANNELS.personCatalogDeleted,

  'mika:activity': MIKA_EVENT_CHANNELS.activity,

  'privacy:senses-paused': MIKA_EVENT_CHANNELS.sensesPaused,
} as const

export type MikaBridgedEventName = keyof typeof MIKA_BRIDGED_EVENT_CHANNELS

export const MIKA_BRIDGED_EVENTS = Object.keys(MIKA_BRIDGED_EVENT_CHANNELS) as MikaBridgedEventName[]

export function getMikaEventChannel(eventName: MikaBridgedEventName): string {
  return MIKA_BRIDGED_EVENT_CHANNELS[eventName]
}
