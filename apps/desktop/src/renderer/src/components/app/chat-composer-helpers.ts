import type { TranslationDict } from '@mika/sdk/i18n'
import type { SlashCommandDef } from '../../hooks/shell-commands'
import { slashCommandOptionId } from '../../hooks/useSlashCommandSuggestions'

export const MAX_MSG_LENGTH = 2000

export const QUICK_ACTIONS: ReadonlyArray<{ label: keyof TranslationDict; msg: keyof TranslationDict }> = [
  { label: 'shell.composer.quick.screen', msg: 'shell.composer.quick.screenMsg' },
  { label: 'shell.composer.quick.game', msg: 'shell.composer.quick.gameMsg' },
  { label: 'shell.composer.quick.who', msg: 'shell.composer.quick.whoMsg' },
  { label: 'shell.composer.quick.react', msg: 'shell.composer.quick.reactMsg' },
  { label: 'shell.composer.quick.surprise', msg: 'shell.composer.quick.surpriseMsg' },
]

export const PLACEHOLDERS: ReadonlyArray<keyof TranslationDict> = [
  'shell.composer.ph1',
  'shell.composer.ph2',
  'shell.composer.ph3',
  'shell.composer.ph4',
  'shell.composer.ph5',
  'shell.composer.ph6',
]

export function getComposerFormVisuals(listening: boolean, isFocused: boolean) {
  const borderClass = listening
    ? 'border-danger-rim shadow-[0_0_48px_color-mix(in_srgb,var(--danger)_15%,transparent),0_24px_64px_rgba(0,0,0,0.55)]'
    : isFocused
      ? 'border-info-rim shadow-[0_0_48px_color-mix(in_srgb,var(--info)_12%,transparent),0_24px_64px_rgba(0,0,0,0.55)]'
      : 'border-border-subtle shadow-[0_20px_60px_rgba(0,0,0,0.45)]'
  const gradient = listening
    ? 'linear-gradient(90deg, transparent 0%, color-mix(in srgb, var(--danger) 80%, transparent) 50%, transparent 100%)'
    : 'linear-gradient(90deg, transparent 0%, var(--neural-purple) 30%, var(--neural-cyan) 70%, transparent 100%)'
  return { borderClass, gradientTop: gradient, gradientBottom: gradient }
}

export function resolveActiveSlashOptionId(matches: SlashCommandDef[], selectedIndex: number): string | undefined {
  const active = matches[selectedIndex]
  return active ? slashCommandOptionId(active.command) : undefined
}
