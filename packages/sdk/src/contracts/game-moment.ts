import { z } from 'zod'

export const GameMomentTypeSchema = z.enum([
  'death',
  'achievement',
  'boss',
  'cutscene',
  'levelup',
  'victory',
  'defeat',
  'discovery',
])
export type GameMomentType = z.infer<typeof GameMomentTypeSchema>

export const GAME_MOMENT_TO_SESSION_EVENT_TYPE: Record<
  GameMomentType,
  'cutscene' | 'combat' | 'exploration' | 'dialogue' | 'death' | 'achievement' | 'screenshot'
> = {
  death: 'death',
  achievement: 'achievement',
  cutscene: 'cutscene',
  boss: 'combat',
  victory: 'combat',
  defeat: 'combat',
  levelup: 'exploration',
  discovery: 'exploration',
}
