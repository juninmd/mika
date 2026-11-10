/**
 * Densidade de render do avatar e dos efeitos — fonte ÚNICA. O teto de 2x vivia copiado em quatro
 * lugares (Pixi na criação, Pixi no resize, three, canvas de efeitos), e nenhum deles ouvia a troca
 * de monitor: arrastar a janela de uma tela 1x para uma 2x não dispara `resize` quando o tamanho em
 * px CSS não muda, e o avatar ficava borrado (ou caro demais, no sentido contrário) até o próximo
 * redimensionamento manual.
 *
 * `character.renderSharpness` (schema em `config/schemas/character.rs`, controle na aba Skin) deixa
 * o usuário trocar nitidez por GPU: a Mika divide a placa com o jogo.
 */

/** Mesmos valores e mesma ordem do `choice("renderSharpness", ...)` do schema Rust (teste lá confere). */
export const RENDER_SHARPNESS_MODES = ['auto', 'performance', 'sharp'] as const
export type RenderSharpness = (typeof RENDER_SHARPNESS_MODES)[number]
export const DEFAULT_RENDER_SHARPNESS: RenderSharpness = 'auto'

/** Acima de 2x o custo de pixel quadruplica sem ganho visível num avatar desse tamanho. */
export const MAX_RENDER_PIXEL_RATIO = 2
/** `sharp` superamostra: desenha acima da densidade do monitor e o navegador reduz, alisando a borda. */
const SHARP_SUPERSAMPLE = 1.5
export const MAX_SHARP_PIXEL_RATIO = 3

export function renderSharpnessFromConfig(value: unknown): RenderSharpness {
  return RENDER_SHARPNESS_MODES.find((mode) => mode === value) ?? DEFAULT_RENDER_SHARPNESS
}

export function renderPixelRatio(devicePixelRatio: number | undefined, sharpness: RenderSharpness): number {
  const dpr = devicePixelRatio || 1
  if (sharpness === 'performance') return Math.min(dpr, 1)
  if (sharpness === 'sharp') return Math.min(dpr * SHARP_SUPERSAMPLE, MAX_SHARP_PIXEL_RATIO)
  return Math.min(dpr, MAX_RENDER_PIXEL_RATIO)
}

let sharpness: RenderSharpness = DEFAULT_RENDER_SHARPNESS
const sharpnessListeners = new Set<() => void>()

export function setRenderSharpness(next: RenderSharpness): void {
  if (sharpness === next) return
  sharpness = next
  for (const notify of sharpnessListeners) notify()
}

/** Densidade que os canvases devem usar AGORA: monitor atual + modo escolhido. */
export function currentRenderPixelRatio(): number {
  return renderPixelRatio(window.devicePixelRatio, sharpness)
}

/**
 * Chama `onChange` quando a densidade de render muda: troca de monitor ou de modo de nitidez. A
 * consulta `(resolution: Ndppx)` só dispara ao SAIR de N, então é rearmada para a densidade nova a
 * cada troca — sem isso o aviso funciona uma única vez. Devolve o descarte.
 */
export function watchRenderPixelRatio(onChange: () => void): () => void {
  sharpnessListeners.add(onChange)
  let disarm = () => {}
  const arm = () => {
    if (typeof window.matchMedia !== 'function') return
    const query = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`)
    const handle = () => {
      disarm()
      arm()
      onChange()
    }
    query.addEventListener('change', handle)
    disarm = () => query.removeEventListener('change', handle)
  }
  arm()
  return () => {
    sharpnessListeners.delete(onChange)
    disarm()
  }
}
