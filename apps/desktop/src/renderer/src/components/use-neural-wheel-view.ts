import { type Dispatch, type RefObject, type SetStateAction, useEffect } from 'react'

export type WheelView = 'root' | 'games'

export function useNeuralWheelView(
  isOpen: boolean,
  view: WheelView,
  setView: Dispatch<SetStateAction<WheelView>>,
  setActiveIndex: Dispatch<SetStateAction<number | null>>,
  dialogRef: RefObject<HTMLDivElement | null>,
  onClose: () => void,
) {
  useEffect(() => {
    if (isOpen) return
    setView('root')
    setActiveIndex(null)
  }, [isOpen, setView, setActiveIndex])

  useEffect(() => {
    if (!isOpen) return
    // Foco entra no diálogo: sem isto o Tab escapava para trás do backdrop e o teclado não navegava.
    // O alvo muda com a view: no submenu de jogos o primeiro foco é o botão "Voltar". Seletor por
    // data-testid, não por aria-label — o label é traduzido e o seletor por texto quebrava fora do pt-BR.
    const selector = view === 'games' ? '[data-testid="neural-wheel-back"]' : '[data-testid^="neural-wheel-item-"]'
    dialogRef.current?.querySelector<HTMLElement>(selector)?.focus()
  }, [isOpen, view, dialogRef])

  useEffect(() => {
    if (!isOpen) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== 'Escape' && event.key !== 'Backspace') return
      event.preventDefault()
      if (view === 'root') {
        if (event.key === 'Escape') onClose()
        return
      }
      setView('root')
    }
    window.addEventListener('keydown', onKeyDown)
    return () => window.removeEventListener('keydown', onKeyDown)
  }, [isOpen, view, onClose, setView])
}
