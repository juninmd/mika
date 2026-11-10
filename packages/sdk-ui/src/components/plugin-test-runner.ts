import { t } from '@mika/sdk/i18n'
import { toErrorMessage } from '@mika/sdk/utils/object'
import type { TestStatus } from './TestResultNote'

export type PluginTestResponse = { ok: boolean; message?: string; audio?: string }

export type PluginTestRunnerOptions = {
  testFn: () => Promise<PluginTestResponse>
  onAudio?: (base64Audio: string) => void
  resetDelayMs: number
  runningMessage: string
}

export type PluginTestRunner = {
  run: () => Promise<void>
  reset: () => void
  dispose: () => void
}

function resolveMessage(res: PluginTestResponse): string {
  if (res.message) return res.message
  return t(res.ok ? 'sdkUi.pluginTest.ok' : 'sdkUi.pluginTest.failed')
}

async function attempt(
  testFn: PluginTestRunnerOptions['testFn'],
): Promise<{ status: TestStatus; message: string; audio?: string }> {
  try {
    const res = await testFn()
    return { status: res.ok ? 'ok' : 'error', message: resolveMessage(res), audio: res.audio }
  } catch (err) {
    return { status: 'error', message: t('sdkUi.pluginTest.errorWith', { error: toErrorMessage(err) }) }
  }
}

/** Only the latest run may publish its outcome; an older one finishing late is dropped. */
export function createPluginTestRunner(
  options: () => PluginTestRunnerOptions,
  publish: (status: TestStatus, message: string | null) => void,
): PluginTestRunner {
  let timer: ReturnType<typeof setTimeout> | null = null
  let latest = 0

  const clearTimer = () => {
    if (timer) clearTimeout(timer)
    timer = null
  }
  const reset = () => {
    clearTimer()
    publish('idle', null)
  }

  const run = async (): Promise<void> => {
    const { testFn, onAudio, resetDelayMs, runningMessage } = options()
    const id = ++latest
    reset()
    publish('testing', runningMessage)
    const outcome = await attempt(testFn)
    if (id !== latest) return
    if (outcome.audio && onAudio) onAudio(outcome.audio)
    publish(outcome.status, outcome.message)
    if (resetDelayMs > 0) timer = setTimeout(reset, resetDelayMs)
  }

  return { run, reset, dispose: clearTimer }
}
