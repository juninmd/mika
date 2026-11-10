import { type RefObject, useCallback } from 'react'

type SlashMenuControls = {
  isOpen: boolean
  onNavigate: (direction: 'up' | 'down') => void
  onSelect: () => void
  onClose: () => void
}

type UseChatComposerKeyboardOptions = {
  isComposingRef: RefObject<boolean>
  canSend: boolean
  listening: boolean
  stopListening: () => void
  onClose: () => void
  onRecallLastMessage?: () => void
  slashMenu?: SlashMenuControls
}

function isComposingKeyEvent(isComposingRef: RefObject<boolean>, e: React.KeyboardEvent<HTMLTextAreaElement>): boolean {
  return isComposingRef.current || e.nativeEvent?.isComposing || (e as unknown as { keyCode?: number })?.keyCode === 229
}

/** Returns true when the slash-command dropdown consumed the key press. */
function handleSlashMenuKey(e: React.KeyboardEvent<HTMLTextAreaElement>, slashMenu: SlashMenuControls): boolean {
  if (!slashMenu.isOpen) return false

  if (e.key === 'ArrowDown') {
    e.preventDefault()
    slashMenu.onNavigate('down')
    return true
  }
  if (e.key === 'ArrowUp') {
    e.preventDefault()
    slashMenu.onNavigate('up')
    return true
  }
  if (e.key === 'Tab' || e.key === 'Enter') {
    e.preventDefault()
    slashMenu.onSelect()
    return true
  }
  if (e.key === 'Escape') {
    e.preventDefault()
    slashMenu.onClose()
    return true
  }
  return false
}

type BaseKeyHandlers = {
  canSend: boolean
  listening: boolean
  stopListening: () => void
  onClose: () => void
  onRecallLastMessage?: () => void
}

/** The composer's baseline shortcuts (Enter/ArrowUp/Escape) once no slash menu intercepted the key. */
function handleBaseComposerKey(e: React.KeyboardEvent<HTMLTextAreaElement>, opts: BaseKeyHandlers): void {
  if (e.key === 'Enter' && !e.shiftKey && opts.canSend) {
    e.preventDefault()
    e.currentTarget.form?.requestSubmit()
    return
  }

  if (e.key === 'ArrowUp' && !e.currentTarget.value && opts.onRecallLastMessage) {
    e.preventDefault()
    opts.onRecallLastMessage()
    return
  }

  if (e.key === 'Escape') {
    if (opts.listening) {
      opts.stopListening()
    } else {
      opts.onClose()
    }
  }
}

export function useChatComposerKeyboard({
  isComposingRef,
  canSend,
  listening,
  stopListening,
  onClose,
  onRecallLastMessage,
  slashMenu,
}: UseChatComposerKeyboardOptions) {
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (isComposingKeyEvent(isComposingRef, e)) return
      if (slashMenu && handleSlashMenuKey(e, slashMenu)) return

      handleBaseComposerKey(e, { canSend, listening, stopListening, onClose, onRecallLastMessage })
    },
    [isComposingRef, canSend, listening, stopListening, onClose, onRecallLastMessage, slashMenu],
  )

  return { handleKeyDown }
}
