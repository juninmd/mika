import type { Translate } from '../../src/hooks/useI18n'
import type { SliderControl, ToggleControl } from './vision-settings-components'

type CaptureValues = {
  captureQuality: number
  llmQuality: number
  maxImageSizeKB: number
  compressionTargetSizeKB: number
  maxImageDimension: number
  maxOutputTokens: number
  captureIntervalMs: number
  proactiveCommentaryChance: number
}

export function getCaptureSliders(
  t: Translate,
  {
    captureQuality,
    llmQuality,
    maxImageSizeKB,
    compressionTargetSizeKB,
    maxImageDimension,
    maxOutputTokens,
    captureIntervalMs,
    proactiveCommentaryChance,
  }: CaptureValues,
): SliderControl[] {
  return [
    {
      label: t('settings.vision.field.captureQuality.label'),
      configPath: 'vision.captureQuality',
      value: captureQuality,
      min: 1,
      max: 100,
      showValue: (v) => `${v}%`,
    },
    {
      label: t('settings.vision.panel.llmQualityLabel'),
      configPath: 'vision.llmQuality',
      value: llmQuality,
      min: 1,
      max: 100,
      showValue: (v) => `${v}%`,
    },
    {
      label: t('settings.vision.panel.maxSizeLabel'),
      configPath: 'vision.maxImageSizeKB',
      value: maxImageSizeKB,
      min: 50,
      max: 2000,
      step: 50,
      showValue: (v) => `${v} KB`,
    },
    {
      label: t('settings.vision.panel.compressionTargetLabel'),
      configPath: 'vision.compressionTargetSizeKB',
      value: compressionTargetSizeKB,
      min: 50,
      max: 1500,
      step: 50,
      showValue: (v) => `${v} KB`,
    },
    {
      label: t('settings.vision.panel.maxDimensionLabel'),
      configPath: 'vision.maxImageDimension',
      value: maxImageDimension,
      min: 320,
      max: 4096,
      step: 64,
      showValue: (v) => `${v}px`,
    },
    {
      label: t('settings.vision.panel.maxOutputTokensLabel'),
      configPath: 'vision.maxOutputTokens',
      value: maxOutputTokens,
      min: 128,
      max: 4096,
      step: 64,
      showValue: (v) => `${v}`,
    },
    {
      label: t('settings.vision.panel.captureIntervalLabel'),
      configPath: 'vision.captureIntervalMs',
      value: captureIntervalMs,
      min: 60000,
      max: 600000,
      step: 5000,
      showValue: (v) => (v >= 1000 ? `${(v / 1000).toFixed(0)}s` : `${v}ms`),
    },
    {
      label: t('settings.vision.panel.proactiveCommentaryLabel'),
      configPath: 'vision.proactiveCommentaryChance',
      value: proactiveCommentaryChance,
      min: 0,
      max: 1,
      step: 0.05,
      showValue: (v) => `${(v * 100).toFixed(0)}%`,
    },
  ]
}

export function getBehaviorToggles(
  t: Translate,
  { autoDescribeScreen, visionAssistActive }: { autoDescribeScreen: boolean; visionAssistActive: boolean },
): ToggleControl[] {
  return [
    {
      label: t('settings.vision.panel.autoDescribeLabel'),
      configPath: 'vision.autoDescribeScreen',
      value: autoDescribeScreen,
    },
    {
      label: t('settings.vision.panel.visionAssistLabel'),
      configPath: 'vision.visionAssistActive',
      value: visionAssistActive,
    },
  ]
}

type AwarenessValues = {
  codeAwareness: boolean
  gameAwareness: boolean
  readClipboardImages: boolean
  visualDiaryEnabled: boolean
}

export function getAwarenessToggles(
  t: Translate,
  { codeAwareness, gameAwareness, readClipboardImages, visualDiaryEnabled }: AwarenessValues,
): ToggleControl[] {
  return [
    { label: t('settings.vision.panel.codeAwarenessLabel'), configPath: 'vision.codeAwareness', value: codeAwareness },
    { label: t('settings.vision.panel.gameAwarenessLabel'), configPath: 'vision.gameAwareness', value: gameAwareness },
    {
      label: t('settings.vision.panel.clipboardImagesLabel'),
      configPath: 'vision.readClipboardImages',
      value: readClipboardImages,
    },
    {
      label: t('settings.vision.panel.visualDiaryLabel'),
      configPath: 'vision.visualDiaryEnabled',
      value: visualDiaryEnabled,
    },
  ]
}
