// GERADO POR scripts/live2d-gen-motions.mts — NÃO EDITE À MÃO.
// Regenerar: pnpm live2d:gen · Conferir: pnpm check:live2d-motions
/**
 * Ações disponíveis por modelo, com o índice delas no grupo `Action` do `model3.json`.
 *
 * `params` é a máscara: enquanto a ação toca, o animador procedural NÃO escreve nesses parâmetros.
 * Sem a máscara a motion é calculada e sobrescrita no mesmo frame, e o efeito visível é zero.
 */
export interface Live2DActionEntry {
  /** Posição no grupo `Action` — é por índice que o pixi-live2d-display toca. */
  readonly index: number
  readonly duration: number
  readonly fadeIn: number
  readonly fadeOut: number
  /** Parâmetros que esta motion dirige. O animador cede estes canais enquanto ela toca. */
  readonly params: readonly string[]
}

/** Chave externa: o nome do arquivo `.model3.json` sem extensão (`Mao`, `haru_greeter_t05`, ...). */
export const LIVE2D_ACTION_CATALOG: Readonly<Record<string, Readonly<Record<string, Live2DActionEntry>>>> = {
  "Mao": {
    happy: { index: 0, duration: 3, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleY', 'ParamAngleZ', 'ParamBodyAngleY', 'ParamBodyAngleZ', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamCheek', 'ParamA'] },
    wink: { index: 1, duration: 2, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamEyeROpen', 'ParamEyeRSmile', 'ParamBrowRY', 'ParamAngleX', 'ParamAngleZ', 'ParamBodyAngleZ', 'ParamA'] },
    nod: { index: 2, duration: 2.2, fadeIn: 0.4, fadeOut: 0.5, params: ['ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY'] },
    shakehead: { index: 3, duration: 2.4, fadeIn: 0.4, fadeOut: 0.5, params: ['ParamAngleX', 'ParamBodyAngleX', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLForm', 'ParamBrowRForm'] },
    thinking: { index: 4, duration: 4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleX', 'ParamAngleY', 'ParamAngleZ', 'ParamEyeBallX', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamBrowLAngle', 'ParamBrowRAngle', 'ParamBodyAngleZ'] },
    surprised: { index: 5, duration: 2.5, fadeIn: 0.2, fadeOut: 0.5, params: ['ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamA', 'ParamAngleY', 'ParamBodyAngleY'] },
    shy: { index: 6, duration: 4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamCheek', 'ParamAngleX', 'ParamAngleY', 'ParamAngleZ', 'ParamEyeBallX', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamA', 'ParamBodyAngleZ'] },
    angry: { index: 7, duration: 2.6, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamA', 'ParamAngleY', 'ParamAngleX', 'ParamBodyAngleY'] },
    sad: { index: 8, duration: 3.4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleY', 'ParamAngleZ', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamBodyAngleY'] },
    wave: { index: 9, duration: 2.8, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamArmRA01', 'ParamArmRB01', 'ParamArmRA02', 'ParamArmRB02', 'ParamArmRA03', 'ParamArmRB03', 'ParamAngleZ', 'ParamBodyAngleZ', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamA'] },
    shrug: { index: 10, duration: 2.4, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamArmLA01', 'ParamArmRA01', 'ParamArmLB01', 'ParamArmRB01', 'ParamArmLA03', 'ParamArmRA03', 'ParamArmLB03', 'ParamArmRB03', 'ParamAngleZ', 'ParamAngleY', 'ParamBrowLY', 'ParamBrowRY'] },
    cheer: { index: 11, duration: 2.6, fadeIn: 0.2, fadeOut: 0.5, params: ['ParamArmLA01', 'ParamArmRA01', 'ParamArmLB01', 'ParamArmRB01', 'ParamArmLA02', 'ParamArmRA02', 'ParamArmLB02', 'ParamArmRB02', 'ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamA', 'ParamBrowLY', 'ParamBrowRY'] },
    laugh: { index: 12, duration: 2.4, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamA'] },
    sleepy: { index: 13, duration: 3.2, fadeIn: 0.6, fadeOut: 0.8, params: ['ParamAngleY', 'ParamAngleZ', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeBallY', 'ParamBrowLY', 'ParamBrowRY', 'ParamA'] },
  },
  "haru_greeter_t05": {
    happy: { index: 0, duration: 3, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleY', 'ParamAngleZ', 'ParamBodyAngleY', 'ParamBodyAngleZ', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamTere', 'ParamMouthOpenY'] },
    wink: { index: 1, duration: 2, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamEyeROpen', 'ParamEyeRSmile', 'ParamBrowRY', 'ParamAngleX', 'ParamAngleZ', 'ParamBodyAngleZ', 'ParamMouthOpenY'] },
    nod: { index: 2, duration: 2.2, fadeIn: 0.4, fadeOut: 0.5, params: ['ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY'] },
    shakehead: { index: 3, duration: 2.4, fadeIn: 0.4, fadeOut: 0.5, params: ['ParamAngleX', 'ParamBodyAngleX', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamMouthForm'] },
    thinking: { index: 4, duration: 4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleX', 'ParamAngleY', 'ParamAngleZ', 'ParamEyeBallX', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamBrowLAngle', 'ParamBrowRAngle', 'ParamMouthForm', 'ParamBodyAngleZ'] },
    surprised: { index: 5, duration: 2.5, fadeIn: 0.2, fadeOut: 0.5, params: ['ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamMouthForm', 'ParamMouthOpenY', 'ParamAngleY', 'ParamBodyAngleY'] },
    shy: { index: 6, duration: 4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamTere', 'ParamAngleX', 'ParamAngleY', 'ParamAngleZ', 'ParamEyeBallX', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamMouthForm', 'ParamMouthOpenY', 'ParamBodyAngleZ'] },
    angry: { index: 7, duration: 2.6, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamMouthForm', 'ParamMouthOpenY', 'ParamAngleY', 'ParamAngleX', 'ParamBodyAngleY'] },
    sad: { index: 8, duration: 3.4, fadeIn: 0.5, fadeOut: 0.5, params: ['ParamAngleY', 'ParamAngleZ', 'ParamEyeBallY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLForm', 'ParamBrowRForm', 'ParamMouthForm', 'ParamBodyAngleY'] },
    wave: { index: 9, duration: 2.8, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamArmRB', 'ParamHandAngleR', 'ParamAngleZ', 'ParamBodyAngleZ', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamMouthOpenY'] },
    shrug: { index: 10, duration: 2.4, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamArmLB', 'ParamArmRB', 'ParamHandAngleL', 'ParamHandAngleR', 'ParamAngleZ', 'ParamAngleY', 'ParamBrowLY', 'ParamBrowRY', 'ParamMouthForm'] },
    cheer: { index: 11, duration: 2.6, fadeIn: 0.2, fadeOut: 0.5, params: ['ParamArmLB', 'ParamArmRB', 'ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamMouthOpenY', 'ParamBrowLY', 'ParamBrowRY'] },
    laugh: { index: 12, duration: 2.4, fadeIn: 0.3, fadeOut: 0.5, params: ['ParamAngleY', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeLSmile', 'ParamEyeRSmile', 'ParamMouthOpenY', 'ParamMouthForm'] },
    pout: { index: 13, duration: 2.2, fadeIn: 0.4, fadeOut: 0.5, params: ['ParamAngleX', 'ParamAngleZ', 'ParamEyeBallX', 'ParamBrowLY', 'ParamBrowRY', 'ParamBrowLAngle', 'ParamBrowRAngle', 'ParamMouthForm', 'ParamTere'] },
    sleepy: { index: 14, duration: 3.2, fadeIn: 0.6, fadeOut: 0.8, params: ['ParamAngleY', 'ParamAngleZ', 'ParamBodyAngleY', 'ParamEyeLOpen', 'ParamEyeROpen', 'ParamEyeBallY', 'ParamBrowLY', 'ParamBrowRY', 'ParamMouthOpenY'] },
  },
}

export const LIVE2D_ACTION_GROUP = 'Action'
