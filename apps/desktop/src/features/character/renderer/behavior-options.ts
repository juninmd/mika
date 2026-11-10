import { choiceOption } from '@mika/sdk-ui/settings'
import {
  Bookmark,
  BrainCircuit,
  BriefcaseBusiness,
  Coffee,
  Drama,
  Flame,
  Glasses,
  Heart,
  History as HistoryIcon,
  Laugh,
  Leaf,
  Lightbulb,
  Meh,
  MessageSquare,
  MessagesSquare,
  PartyPopper,
  Scale,
  ScrollText,
  Shield,
  ShieldCheck,
  SignalHigh,
  SignalLow,
  SignalMedium,
  Tornado,
  Zap,
} from 'lucide-react'
import type { Translate } from '../../../renderer/src/hooks/useI18n'

export function moodOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.moodBaseline.option.calm'),
      'calm',
      t('settings.character.field.moodBaseline.hint.calm'),
      Leaf,
    ),
    choiceOption(
      t('settings.character.field.moodBaseline.option.energetic'),
      'energetic',
      t('settings.character.field.moodBaseline.hint.energetic'),
      Zap,
    ),
    choiceOption(
      t('settings.character.field.moodBaseline.option.playful'),
      'playful',
      t('settings.character.field.moodBaseline.hint.playful'),
      PartyPopper,
    ),
    choiceOption(
      t('settings.character.field.moodBaseline.option.serious'),
      'serious',
      t('settings.character.field.moodBaseline.hint.serious'),
      Glasses,
    ),
    choiceOption(
      t('settings.character.field.moodBaseline.option.chaotic'),
      'chaotic',
      t('settings.character.field.moodBaseline.hint.chaotic'),
      Tornado,
    ),
  ]
}

export function formalityOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.formalityLevel.option.casual'),
      'casual',
      t('settings.character.field.formalityLevel.hint.casual'),
      Coffee,
    ),
    choiceOption(
      t('settings.character.field.formalityLevel.option.balanced'),
      'balanced',
      t('settings.character.field.formalityLevel.hint.balanced'),
      Scale,
    ),
    choiceOption(
      t('settings.character.field.formalityLevel.option.formal'),
      'formal',
      t('settings.character.field.formalityLevel.hint.formal'),
      BriefcaseBusiness,
    ),
  ]
}

export function humorOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.humorStyle.option.dry'),
      'dry',
      t('settings.character.field.humorStyle.hint.dry'),
      Meh,
    ),
    choiceOption(
      t('settings.character.field.humorStyle.option.witty'),
      'witty',
      t('settings.character.field.humorStyle.hint.witty'),
      Lightbulb,
    ),
    choiceOption(
      t('settings.character.field.humorStyle.option.silly'),
      'silly',
      t('settings.character.field.humorStyle.hint.silly'),
      Laugh,
    ),
    choiceOption(
      t('settings.character.field.humorStyle.option.sarcastic'),
      'sarcastic',
      t('settings.character.field.humorStyle.hint.sarcastic'),
      Drama,
    ),
    choiceOption(
      t('settings.character.field.humorStyle.option.wholesome'),
      'wholesome',
      t('settings.character.field.humorStyle.hint.wholesome'),
      Heart,
    ),
  ]
}

export function jokeOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.jokeFrequency.option.rare'),
      'rare',
      t('settings.character.field.jokeFrequency.hint.rare'),
      SignalLow,
    ),
    choiceOption(
      t('settings.character.field.jokeFrequency.option.occasional'),
      'occasional',
      t('settings.character.field.jokeFrequency.hint.occasional'),
      SignalMedium,
    ),
    choiceOption(
      t('settings.character.field.jokeFrequency.option.frequent'),
      'frequent',
      t('settings.character.field.jokeFrequency.hint.frequent'),
      SignalHigh,
    ),
  ]
}

export function swearOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.swearTolerance.option.none'),
      'none',
      t('settings.character.field.swearTolerance.hint.none'),
      ShieldCheck,
    ),
    choiceOption(
      t('settings.character.field.swearTolerance.option.mild'),
      'mild',
      t('settings.character.field.swearTolerance.hint.mild'),
      Shield,
    ),
    choiceOption(
      t('settings.character.field.swearTolerance.option.moderate'),
      'moderate',
      t('settings.character.field.swearTolerance.hint.moderate'),
      Flame,
    ),
  ]
}

export function lengthOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.field.responseLength.option.concise'),
      'concise',
      t('settings.character.field.responseLength.hint.concise'),
      MessageSquare,
    ),
    choiceOption(
      t('settings.character.field.responseLength.option.balanced'),
      'balanced',
      t('settings.character.field.responseLength.hint.balanced'),
      MessagesSquare,
    ),
    choiceOption(
      t('settings.character.field.responseLength.option.detailed'),
      'detailed',
      t('settings.character.field.responseLength.hint.detailed'),
      ScrollText,
    ),
  ]
}

export function memoryOptions(t: Translate) {
  return [
    choiceOption(
      t('settings.character.behaviorPanel.option.memorySelective'),
      'selective',
      t('settings.character.behaviorPanel.hint.memorySelective'),
      Bookmark,
    ),
    choiceOption(
      t('settings.character.behaviorPanel.option.memoryEager'),
      'eager',
      t('settings.character.behaviorPanel.hint.memoryEager'),
      HistoryIcon,
    ),
    choiceOption(
      t('settings.character.behaviorPanel.option.memoryObsessive'),
      'obsessive',
      t('settings.character.behaviorPanel.hint.memoryObsessive'),
      BrainCircuit,
    ),
  ]
}

export const BEHAVIOR_FLOW = `flowchart LR
  A[Input: mensagem + canal + anexos] --> B[Contexto: histórico + resumo + memórias + humor + tela]
  B --> C{LLM decide}
  C -->|usa ferramentas| D[Tool loop com aprovação]
  D --> C
  C --> E[Sanitiza texto + detecta emoção]
  E --> F[Resposta: texto + TTS + expressão]
  F -.->|pós-turno: fatos + memórias| B`
