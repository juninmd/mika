import { useCallback, useEffect, useRef, useState } from 'react'
import { createPluginTestRunner, type PluginTestResponse } from './plugin-test-runner'
import type { TestStatus } from './TestResultNote'

export interface UsePluginTestOptions {
  testFn: () => Promise<PluginTestResponse>
  onAudio?: (base64Audio: string) => void
  resetDelayMs?: number
  runningMessage?: string
}

export interface UsePluginTestResult {
  status: TestStatus
  message: string | null
  isTesting: boolean
  runTest: () => Promise<void>
  reset: () => void
}

export function usePluginTest({
  testFn,
  onAudio,
  resetDelayMs = 3000,
  runningMessage = 'Testando...',
}: UsePluginTestOptions): UsePluginTestResult {
  const [state, setState] = useState<{ status: TestStatus; message: string | null }>({ status: 'idle', message: null })
  const optionsRef = useRef({ testFn, onAudio, resetDelayMs, runningMessage })
  optionsRef.current = { testFn, onAudio, resetDelayMs, runningMessage }
  const runnerRef = useRef<ReturnType<typeof createPluginTestRunner> | null>(null)
  runnerRef.current ??= createPluginTestRunner(
    () => optionsRef.current,
    (status, message) => setState({ status, message }),
  )
  const runner = runnerRef.current

  useEffect(() => runner.dispose, [runner])

  const runTest = useCallback(() => runner.run(), [runner])
  const reset = useCallback(() => runner.reset(), [runner])

  return { status: state.status, message: state.message, isTesting: state.status === 'testing', runTest, reset }
}
