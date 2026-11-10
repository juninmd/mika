import { type TranslationDict, t } from '@mika/sdk/i18n'

const TOOL_LABEL_KEYS: Record<string, keyof TranslationDict> = {
  vision_capture: 'shell.toolLabel.vision_capture',
  vision_read: 'shell.toolLabel.vision_read',
  person_catalog: 'shell.toolLabel.person_catalog',
  steam: 'shell.toolLabel.steam',
  game_knowledge: 'shell.toolLabel.game_knowledge',
  recommend_media: 'shell.toolLabel.recommend_media',
  user_profile: 'shell.toolLabel.user_profile',
  mood: 'shell.toolLabel.mood',
  track_gamer_mood: 'shell.toolLabel.track_gamer_mood',
  get_system_info: 'shell.toolLabel.get_system_info',
  memory_console: 'shell.toolLabel.memory_console',
  content: 'shell.toolLabel.content',
  collaboration: 'shell.toolLabel.collaboration',
  match_effect: 'shell.toolLabel.match_effect',
  avatar: 'shell.toolLabel.avatar',
  game_session: 'shell.toolLabel.game_session',
  play_minigame: 'shell.toolLabel.play_minigame',
  clear_history: 'shell.toolLabel.clear_history',
  voice: 'shell.toolLabel.voice',
  desktop: 'shell.toolLabel.desktop',
  reminder: 'shell.toolLabel.reminder',
}

/** Localized "what she is doing" line for a tool; an unmapped tool keeps its raw name. */
export function getFriendlyToolLabel(toolName: string): string {
  const key = TOOL_LABEL_KEYS[toolName]
  return key ? t(key) : toolName
}
