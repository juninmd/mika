import './lib/expose-plugin-globals'
import { MotionConfig } from 'framer-motion'
import React, { lazy, Suspense } from 'react'
import ReactDOM from 'react-dom/client'
import { AppBoundary } from './components/AppBoundary'
import { ShellFallback } from './components/app/ShellFallbacks'
import './styles.css'
import { useI18n } from './hooks/useI18n'
import { startLongTaskProbe } from './lib/long-task-probe'

const DesktopApp = lazy(() => import('./App').then((m) => ({ default: m.App })))
const SettingsApp = lazy(() => import('./SettingsApp').then((m) => ({ default: m.SettingsApp })))
const PluginViewApp = lazy(() => import('./PluginViewApp').then((m) => ({ default: m.PluginViewApp })))
const WizardApp = lazy(() => import('./WizardApp').then((m) => ({ default: m.WizardApp })))

startLongTaskProbe()

const isSettings = window.location.hash.startsWith('#/settings')
const isWizard = window.location.hash.startsWith('#/wizard')
const isPluginView = new URLSearchParams(window.location.search).get('type') === 'plugin-view'
const hasElectronBridge = Boolean((window as unknown as { electronAPI?: unknown }).electronAPI)

document.documentElement.dataset.surface = isPluginView
  ? 'plugin-view'
  : isSettings
    ? 'settings'
    : isWizard
      ? 'wizard'
      : 'overlay'

function Root() {
  const { t } = useI18n()
  if (!hasElectronBridge && !isSettings && !isPluginView && !isWizard) {
    return (
      <ShellFallback
        title={t('shell.fallback.bridgeUnavailableTitle')}
        message={t('shell.fallback.bridgeUnavailableMessage')}
      />
    )
  }
  if (isPluginView)
    return (
      <Suspense fallback={<BootSplash />}>
        <PluginViewApp />
      </Suspense>
    )
  if (isSettings)
    return (
      <Suspense fallback={<BootSplash />}>
        <SettingsApp />
      </Suspense>
    )
  if (isWizard)
    return (
      <Suspense fallback={<BootSplash />}>
        <WizardApp onComplete={() => window.close()} />
      </Suspense>
    )
  return (
    <AppBoundary surface="desktop-shell">
      <Suspense fallback={<BootSplash />}>
        <DesktopApp />
      </Suspense>
    </AppBoundary>
  )
}

function BootSplash() {
  const { t } = useI18n()
  return <ShellFallback title={t('shell.fallback.bootingTitle')} message={t('shell.fallback.bootingMessage')} busy />
}

const root = document.getElementById('root')
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      {/* O CSS de reduced-motion zera transition/animation, mas não alcança o framer-motion (anima via
          JS/rAF): sem isto, cada entrada `y: 10` e cada hover-lift continuavam a deslizar. */}
      <MotionConfig reducedMotion="user">
        <AppBoundary surface="root">
          <Root />
        </AppBoundary>
      </MotionConfig>
    </React.StrictMode>,
  )
}
