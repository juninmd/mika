import { getLocale, t } from '@mika/sdk/i18n'
import { Component, type ErrorInfo, type ReactNode } from 'react'
import { rendererLog } from '../../lib/renderer-logger'

type Props = { children: ReactNode }
type State = { failed: boolean; message: string }

export class AvatarStageBoundary extends Component<Props, State> {
  state: State = { failed: false, message: '' }

  static getDerivedStateFromError(error: Error): State {
    return { failed: true, message: error.message }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    rendererLog.error('[AvatarStage] palco do avatar falhou; seguindo sem a personagem', {
      error: error.message,
      componentStack: info.componentStack,
    })
  }

  render(): ReactNode {
    if (this.state.failed) {
      return <div role="status" aria-label={t('shell.avatar.boundaryFailed', getLocale())} className="sr-only" />
    }
    return this.props.children
  }
}
