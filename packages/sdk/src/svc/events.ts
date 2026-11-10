// 2026-09-22: 19 chaves removidas por `check:events` (audit pós-migração Tauri) — nem o TS nem o
// Rust as referenciam mais (`POWER_SUSPEND`, `USER_MESSAGE`, `TTS_FALLBACK`, `REFLECTED`,
// `BRAIN_GAMING_TICK`, `BEHAVIOR_STATE_CHANGED`, `VISION_CAPTURE_REQUEST`,
// `PERSON_MENTION_DETECTED`, `PERSON_CREATED`, `GAME_LAUNCHED`, `GAME_SESSION_ENDED`,
// `SCREEN_ENTITIES`, `LLM_PROVIDER_HEALTH`, `PLAY_SOUND`, `PERSON_CATALOG_AUTO_UPSERT`,
// `VISION_SCREENSHOT_SHORTCUT`, `BRAIN_FACT_DISCOVERED`, `VISION_PERSON_DETECTED`,
// `BROWSER_NAVIGATING`). A maioria era CRUD/detecção que virou chamada de função direta dentro do
// mesmo processo Rust (não precisa mais de evento cross-processo); `TTS_FALLBACK` e
// `BROWSER_NAVIGATING` eram diferentes — nenhum dos dois tinha publicador Rust. `BROWSER_NAVIGATING`
// segue fora (o módulo `browser` em Rust não tem motor de navegação, `reqwest`+`scraper`, sem view
// navegável). `TTS_FALLBACK` voltou em 2026-09-25: `voice/tts/dispatch.rs` agora publica de verdade
// quando o ElevenLabs cai para a voz local (`EventKey::TtsFallback`, `events/keys.rs`) — o mesmo
// "sem chamador real" que motivou a remoção deixou de valer assim que o chamador passou a existir.
// Reintroduzir qualquer outra chave exige o mesmo: o emissor real primeiro, nunca a chave sozinha —
// crescer o catálogo especulativamente é o próprio defeito que este gate existe para pegar.
export const Events = {
  OS_ACTIVE_WINDOW_CHANGED: 'os:active-window-changed',

  POWER_RESUME: 'system:power-resume',
  POWER_BATTERY: 'system:power-battery',
  POWER_AC: 'system:power-ac',
  HARDWARE_ALERT: 'hardware:alert',
  SENSES_PAUSED: 'privacy:senses-paused',
  THEME_CHANGED: 'system:theme-changed',

  TOOL_CALL: 'chat:tool-call',
  TOOL_RESULT: 'chat:tool-result',
  STREAM_CHUNK: 'chat:stream-chunk',
  THINKING_START: 'chat:thinking-start',
  THINKING_STOP: 'chat:thinking-stop',
  ASSISTANT_MESSAGE: 'chat:assistant-message',
  ERROR: 'chat:error',

  EXPRESSION: 'avatar:expression',

  SPEECH_STARTED: 'voice:speech-started',
  SPEECH_FINISHED: 'voice:speech-finished',
  TTS_AUDIO_READY: 'voice:tts-audio-ready',
  TTS_FALLBACK: 'voice:tts-fallback',
  LISTENING_STARTED: 'voice:listening-started',
  LISTENING_STOPPED: 'voice:listening-stopped',
  VAD_SAMPLE: 'voice:vad-sample',

  VAD_AUDIO: 'voice:vad-audio',
  SPEECH_REQUEST: 'speech:request',

  BRAIN_PROACTIVE_THOUGHT: 'brain:proactive-thought',

  FORCE_STOP_TTS: 'voice:force-stop-tts',
  INTERRUPTION: 'voice:interruption',

  PERSON_UPDATED: 'person-catalog:updated',
  PERSON_MENTION_ADDED: 'person-catalog:mention-added',
  PERSON_DELETED: 'person-catalog:deleted',

  GAME_CHANGED: 'steam:game-changed',
  GAME_STOPPED: 'steam:game-stopped',
  GAME_MOMENT: 'steam:game-moment',

  SCREEN_OBSERVATION: 'vision:screen-observation',

  NOTIFICATION: 'notification',

  MUTE_SPEECH: 'mika:mute-speech',

  MOOD_CHANGED: 'mood:changed',
  MOOD_AFFINITY_LEVEL_UP: 'mood:affinity-level-up',

  VISION_CLIPBOARD_IMAGE: 'vision:clipboard-image',

  ACTIVITY_CHANGED: 'mika:activity',
} as const

export type EventKey = (typeof Events)[keyof typeof Events]

/** Tool-call origin: `'turn'` is a visible chat turn, `'background'` is post-response enrichment (no "thinking" HUD). */
export type ToolCallOrigin = 'turn' | 'background'
