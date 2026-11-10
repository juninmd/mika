import type { ErrorInfo, ReactNode } from 'react'
import { Component } from 'react'
import { useI18n } from '../hooks/useI18n'
import { rendererLog } from '../lib/renderer-logger'
import { ShellFallback } from './app/ShellFallbacks'

type AppBoundaryProps = {
  children: ReactNode
  surface?: string
}

type AppBoundaryState = {
  error: Error | null
}

function LocalizedAppBoundaryFallback({ error, onAction }: { error: Error; onAction: () => void }) {
  const { t } = useI18n()
  return (
    <ShellFallback
      title={t('shell.boundary.title')}
      message={t('shell.boundary.message')}
      actionLabel={t('shell.boundary.reload')}
      onAction={onAction}
      error={error}
      showStack={process.env.NODE_ENV === 'development'}
    />
  )
}

export class AppBoundary extends Component<AppBoundaryProps, AppBoundaryState> {
  state: AppBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): AppBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    const isDev = process.env.NODE_ENV === 'development'
    rendererLog.error('[RendererBoundary] surface crashed', {
      surface: this.props.surface ?? 'app',
      error: error.message,
      componentStack: info.componentStack,
      isDev,
    })
  }

  private handleReload = (): void => {
    this.setState({ error: null })
    window.location.reload()
  }

  render() {
    if (!this.state.error) return this.props.children

    return <LocalizedAppBoundaryFallback error={this.state.error} onAction={this.handleReload} />
  }
}
