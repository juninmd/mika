import type { FeatureSettingsDeclaration } from '@mika/sdk/contracts'
import { toDisplayError } from '@mika/sdk-ui'
import { useCallback, useMemo, useState } from 'react'
import { useI18n } from '../../hooks/useI18n'
import { getElectronApi } from '../../lib/shell-api'
import {
  buildFeatureValidationPayload,
  getFeatureValidationScenario,
  validationMessages,
} from './feature-validation-scenarios'

type FeatureValidationStatus = 'idle' | 'running' | 'success' | 'error'

export type FeatureValidationState = {
  status: FeatureValidationStatus
  message: string
  /** Wall time of the real check, measured around the IPC call. */
  latencyMs?: number
  testedAt?: number
}

export function useFeatureValidation(feature: FeatureSettingsDeclaration) {
  const { t } = useI18n()
  const scenario = useMemo(() => getFeatureValidationScenario(feature), [feature])
  const payload = useMemo(() => buildFeatureValidationPayload(feature), [feature])
  const [state, setState] = useState<FeatureValidationState>({ status: 'idle', message: '' })

  const validate = useCallback(async () => {
    const messages = validationMessages(scenario, t)
    setState({ status: 'running', message: messages.running })
    const started = performance.now()
    const done = (status: 'success' | 'error', message: string) =>
      setState({ status, message, latencyMs: Math.round(performance.now() - started), testedAt: Date.now() })
    try {
      const result = await getElectronApi().runtime.testPlugin(payload)
      if (result.ok) done('success', result.message || messages.success)
      else done('error', result.message || messages.failure)
    } catch (error) {
      done('error', toDisplayError(error, t('settings.tabHero.test.failed')))
    }
  }, [payload, scenario, t])

  const reset = useCallback(() => setState({ status: 'idle', message: '' }), [])

  return {
    state,
    scenario,
    validate,
    reset,
    running: state.status === 'running',
  }
}
