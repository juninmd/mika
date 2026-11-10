import { getLocale, t } from '@mika/sdk/i18n'
import { Component, type ErrorInfo, type ReactNode } from 'react'
import { rendererLog } from '../lib/renderer-logger'

interface PluginErrorBoundaryProps {
  pluginId?: string
  pluginName: string
  children: ReactNode
  onRetry?: () => void
  onError?: (error: Error, errorInfo: ErrorInfo) => void
}

interface PluginErrorBoundaryState {
  hasError: boolean
  error: Error | null
  errorInfo: ErrorInfo | null
}

export class PluginErrorBoundary extends Component<PluginErrorBoundaryProps, PluginErrorBoundaryState> {
  constructor(props: PluginErrorBoundaryProps) {
    super(props)
    this.state = { hasError: false, error: null, errorInfo: null }
  }

  static getDerivedStateFromError(error: Error): Partial<PluginErrorBoundaryState> {
    return { hasError: true, error }
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo): void {
    const { pluginId, pluginName } = this.props

    rendererLog.error(`[PluginErrorBoundary] Plugin crashed: ${pluginName} (${pluginId ?? 'unknown'})`, {
      error: error.message,
      stack: error.stack,
      componentStack: errorInfo.componentStack,
    })

    this.props.onError?.(error, errorInfo)
  }

  private handleRetry = (): void => {
    this.setState({ hasError: false, error: null, errorInfo: null })
    this.props.onRetry?.()
  }

  render(): ReactNode {
    if (!this.state.hasError) {
      return this.props.children
    }

    const { pluginId, pluginName } = this.props
    const errorMessage = this.state.error?.message ?? t('shell.plugin.unknownError', getLocale())

    return (
      <div
        className="flex h-full w-full items-center justify-center rounded-xl border border-danger-rim bg-danger-soft p-4 text-center"
        role="alert"
      >
        <div className="flex flex-col items-center gap-2">
          <div className="flex items-center gap-2.5 mb-1">
            <span className="text-danger text-lg leading-none shrink-0" aria-hidden="true">
              !
            </span>
            <div className="min-w-0">
              <p className="text-sm font-bold text-danger leading-tight truncate">{pluginName}</p>
              {pluginId && (
                <p className="text-micro font-semibold text-danger uppercase tracking-wider font-mono truncate">
                  {pluginId}
                </p>
              )}
            </div>
          </div>
          <span className="font-mono text-xs text-danger">
            {t('shell.plugin.crashed', getLocale()).replace('{{name}}', pluginName)}
          </span>
          {this.state.error && (
            <details className="w-full max-w-xs">
              <summary className="cursor-pointer text-caption text-danger/80 hover:text-danger focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus rounded">
                {t('shell.plugin.showDetails', getLocale())}
              </summary>
              <pre className="mt-1 max-h-24 overflow-auto rounded bg-surface-raise/40 p-2 text-caption leading-tight text-danger">
                {errorMessage}
              </pre>
            </details>
          )}
          {this.props.onRetry && (
            <button
              type="button"
              onClick={this.handleRetry}
              className="mt-1 rounded-md border border-danger-rim bg-danger-soft px-3 py-1 text-caption font-bold text-danger transition-colors hover:bg-danger hover:text-white focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
            >
              {t('shell.plugin.retry', getLocale())}
            </button>
          )}
        </div>
      </div>
    )
  }
}
