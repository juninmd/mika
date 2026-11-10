import { t } from '@mika/sdk/i18n'
import { describe, expect, it } from 'vitest'
import { getFeatureValidationScenario, validationMessages } from '../feature-validation-scenarios'

const feature = (tabId: string) => ({ tabId, tabLabel: tabId, groups: [] }) as never

describe('feature validation copy', () => {
  // Regression: the running/success/failure sentences were hard-coded Portuguese, so every locale showed them.
  it('is built from the dictionary: each language gets its own sentence around the subject name', () => {
    const scenario = getFeatureValidationScenario(feature('vision'))
    const en = validationMessages(scenario, (key, params) => t(key, params ?? {}, 'en-US'))
    const pt = validationMessages(scenario, (key, params) => t(key, params ?? {}, 'pt-BR'))

    expect(en.running).toContain(t('settings.featureValidation.subject.vision', 'en-US'))
    expect(en.running).not.toBe(pt.running)
    expect(en.success).not.toBe(pt.success)
    expect(en.failure).not.toBe(pt.failure)
    expect(`${en.running}${en.success}${en.failure}`).not.toContain('{{')
  })

  it('unknown tabs fall back to the generic subject', () => {
    expect(getFeatureValidationScenario(feature('whatever')).subjectKey).toBe(
      'settings.featureValidation.subject.default',
    )
  })
})
