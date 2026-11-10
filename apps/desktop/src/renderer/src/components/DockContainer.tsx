import { SPRING } from '@mika/sdk-ui/settings'
import { motion } from 'framer-motion'
import { memo, useEffect, useState } from 'react'
import { useDockActions } from '../hooks/useDockActions'
import { useI18n } from '../hooks/useI18n'
import { useTimeoutRef } from '../hooks/useTransientFlag'
import type { AppStatus } from './app/app-shell.types'
import { CommandDock } from './CommandDock'
import { type BackgroundMode, hudPolicy } from './hud/hud-policy'

type DockContainerProps = {
  backgroundMode: BackgroundMode
  appStatus: AppStatus
  micEnabled: boolean
  vadIsSpeaking: boolean

  pinned: boolean
  showInput: boolean
  showHistory: boolean
  onToggleMic: () => void
  onSetMic?: (enabled: boolean) => void
  onOpenInput: () => void
  onOpenHistory: () => void
  onOpenWheel: () => void
  ttsIsPlaying: boolean
  onStopSpeech: () => void
  sttEnabled?: boolean
  inputMode?: 'ptt' | 'continuous'
  updateAvailable?: boolean
  providerDown?: boolean
}

export const DockContainer = memo(function DockContainer({
  backgroundMode,
  appStatus,
  micEnabled,
  vadIsSpeaking,
  pinned,
  showInput,
  showHistory,
  onToggleMic,
  onSetMic,
  onOpenInput,
  onOpenHistory,
  onOpenWheel,
  ttsIsPlaying,
  onStopSpeech,
  sttEnabled,
  inputMode,
  updateAvailable,
  providerDown,
}: DockContainerProps) {
  const { openSettings } = useDockActions()
  const { t } = useI18n()

  const [isHovered, setIsHovered] = useState(false)
  const [hasFocus, setHasFocus] = useState(false)
  const [shouldHide, setShouldHide] = useState(false)
  const { set: setHideTimer, clear: clearHideTimer } = useTimeoutRef()

  const policy = hudPolicy(backgroundMode)
  const isInteractive = !policy.dockAutoHide || pinned || showInput || showHistory || isHovered || hasFocus

  useEffect(() => {
    const eAPI = (
      window as unknown as {
        electronAPI?: {
          shell?: {
            window?: {
              onMouseMove?: (callback: (pos: { isInside: boolean; insideY: number }) => void) => () => void
            }
          }
        }
      }
    ).electronAPI
    if (eAPI?.shell?.window?.onMouseMove) {
      return eAPI.shell.window.onMouseMove((pos) => {
        const h = window.innerHeight
        if (pos.isInside && pos.insideY > h - 80) {
          setIsHovered(true)
        } else if (!pos.isInside || pos.insideY <= h - 110) {
          setIsHovered(false)
        }
      })
    }
    return undefined
  }, [])

  useEffect(() => {
    const handleGlobalKeys = (e: KeyboardEvent) => {
      // Ctrl+Shift+Space opens the neural wheel (`useNeuralWheel`); here it only brings the dock back.
      // It used to also open the composer, so one keystroke opened two overlays on top of each other.
      if (e.ctrlKey && e.shiftKey && e.code === 'Space') {
        setShouldHide(false)
        setIsHovered(true)
      }
    }
    window.addEventListener('keydown', handleGlobalKeys)
    return () => window.removeEventListener('keydown', handleGlobalKeys)
  }, [])

  useEffect(() => {
    if (isInteractive) {
      setShouldHide(false)
      clearHideTimer()
    } else {
      setHideTimer(() => setShouldHide(true), 3000)
    }
  }, [isInteractive, clearHideTimer, setHideTimer])

  return (
    <div
      className="pointer-events-none flex flex-col items-center justify-end w-screen relative"
      style={{ paddingBottom: 'calc(14px + env(safe-area-inset-bottom, 0px))' }}
    >
      <section
        className="pointer-events-auto relative pt-4 px-8 pb-4"
        aria-label={t('shell.dock.label')}
        onMouseEnter={() => setIsHovered(true)}
        onMouseLeave={(e) => {
          const rect = e.currentTarget.getBoundingClientRect()
          if (e.clientY > rect.bottom - 16) return
          setIsHovered(false)
        }}
        onFocus={(e) => setHasFocus(e.target.matches(':focus-visible'))}
        onBlur={(e) => {
          if (!e.currentTarget.contains(e.relatedTarget)) setHasFocus(false)
        }}
      >
        <div className="pointer-events-auto absolute bottom-[-16px] left-0 right-0 h-4 z-50 cursor-pointer" />
        <motion.div
          animate={{
            y: shouldHide ? 110 : 0,
            opacity: shouldHide ? 0 : 1,
            scale: shouldHide ? 0.95 : 1,
          }}
          initial={false}
          transition={SPRING.soft}
        >
          <CommandDock
            status={appStatus}
            policy={policy}
            micEnabled={micEnabled}
            vadActive={vadIsSpeaking}
            showInput={showInput}
            onToggleMic={onToggleMic}
            onSetMic={onSetMic}
            onOpenInput={onOpenInput}
            onOpenHistory={onOpenHistory}
            onOpenSettings={openSettings}
            onOpenWheel={onOpenWheel}
            onStopSpeech={onStopSpeech}
            ttsIsPlaying={ttsIsPlaying}
            showHistory={showHistory}
            showLegend={isHovered || hasFocus}
            sttEnabled={sttEnabled}
            inputMode={inputMode}
            updateAvailable={updateAvailable}
            providerDown={providerDown}
          />
        </motion.div>
      </section>
    </div>
  )
})
