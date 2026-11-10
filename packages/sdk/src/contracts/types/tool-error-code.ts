export type ToolErrorCode =
  | 'invalid_args'
  | 'not_found'
  | 'not_configured'
  | 'denied'
  | 'unavailable'
  | 'timeout'
  | 'too_large'
  | 'rate_limited'
  | 'transient'
  | 'internal'

export interface ToolErrorPolicy {
  readonly countsAsFailure: boolean

  readonly autoRetry: boolean
}

export const TOOL_ERROR_POLICY: Readonly<Record<ToolErrorCode, ToolErrorPolicy>> = {
  invalid_args: { countsAsFailure: false, autoRetry: false },
  not_found: { countsAsFailure: false, autoRetry: false },
  not_configured: { countsAsFailure: false, autoRetry: false },
  denied: { countsAsFailure: false, autoRetry: false },
  unavailable: { countsAsFailure: true, autoRetry: false },
  timeout: { countsAsFailure: true, autoRetry: false },
  too_large: { countsAsFailure: false, autoRetry: false },
  rate_limited: { countsAsFailure: false, autoRetry: true },
  transient: { countsAsFailure: true, autoRetry: true },
  internal: { countsAsFailure: true, autoRetry: false },
}
