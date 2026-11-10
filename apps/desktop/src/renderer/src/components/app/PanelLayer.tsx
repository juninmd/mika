import { memo } from 'react'
import { ChatHistoryPanel } from '../ChatHistoryPanel'
import type { AppMessage } from './app-shell.types'

interface PanelLayerProps {
  showHistory: boolean
  messages: AppMessage[]
  closeOverlays: () => void
  handleClearHistory: () => void
  onRegenerateLast?: () => void
}

export const PanelLayer = memo(function PanelLayer({
  showHistory,
  messages,
  closeOverlays,
  handleClearHistory,
  onRegenerateLast,
}: PanelLayerProps) {
  return (
    <div className="absolute inset-0 pointer-events-none" style={{ zIndex: 'var(--layer-panels)' }}>
      {showHistory && (
        <div className="pointer-events-auto">
          <ChatHistoryPanel
            messages={messages}
            onClose={closeOverlays}
            onClear={handleClearHistory}
            onRegenerateLast={onRegenerateLast}
          />
        </div>
      )}
    </div>
  )
})
