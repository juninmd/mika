export type ActionKey = readonly [time: number, offset: number]

export interface ActionCurve {
  readonly param: readonly string[]
  readonly keys: readonly ActionKey[]

  readonly unit?: 'absolute' | 'normalized'
}

export interface ActionDef {
  readonly label: string
  readonly duration: number
  readonly fadeIn: number
  readonly fadeOut: number

  readonly requiresAnyOf: readonly (readonly string[])[]
  readonly curves: readonly ActionCurve[]
}

const HEAD_YAW = ['ParamAngleX'] as const
const HEAD_PITCH = ['ParamAngleY'] as const
const HEAD_ROLL = ['ParamAngleZ'] as const
const BODY_YAW = ['ParamBodyAngleX'] as const
const BODY_PITCH = ['ParamBodyAngleY'] as const
const BODY_ROLL = ['ParamBodyAngleZ'] as const
const EYE_OPEN_L = ['ParamEyeLOpen'] as const
const EYE_OPEN_R = ['ParamEyeROpen'] as const
const EYE_SMILE_L = ['ParamEyeLSmile'] as const
const EYE_SMILE_R = ['ParamEyeRSmile'] as const
const EYE_BALL_X = ['ParamEyeBallX'] as const
const EYE_BALL_Y = ['ParamEyeBallY'] as const
const BROW_Y_L = ['ParamBrowLY'] as const
const BROW_Y_R = ['ParamBrowRY'] as const
const BROW_FORM_L = ['ParamBrowLForm'] as const
const BROW_FORM_R = ['ParamBrowRForm'] as const
const BROW_ANGLE_L = ['ParamBrowLAngle'] as const
const BROW_ANGLE_R = ['ParamBrowRAngle'] as const

const MOUTH_OPEN = ['ParamMouthOpenY', 'ParamA'] as const
const MOUTH_FORM = ['ParamMouthForm'] as const

const CHEEK = ['ParamCheek', 'ParamTere'] as const
const SHOULDER = ['ParamShoulder'] as const

const ARM_UPPER_L = ['ParamArmLA01', 'ParamArmL01', 'ParamArmLB', 'ParamArmL'] as const
const ARM_UPPER_R = ['ParamArmRA01', 'ParamArmR01', 'ParamArmRB', 'ParamArmR'] as const
const ARM_UPPER_L_ALT = ['ParamArmLB01'] as const
const ARM_UPPER_R_ALT = ['ParamArmRB01'] as const
const ARM_FORE_L = ['ParamArmLA02', 'ParamArmL02'] as const
const ARM_FORE_R = ['ParamArmRA02', 'ParamArmR02'] as const
const ARM_FORE_L_ALT = ['ParamArmLB02'] as const
const ARM_FORE_R_ALT = ['ParamArmRB02'] as const
const ARM_HAND_L = ['ParamArmLA03', 'ParamArmL03', 'ParamHandLB', 'ParamHandAngleL'] as const
const ARM_HAND_R = ['ParamArmRA03', 'ParamArmR03', 'ParamHandRB', 'ParamHandAngleR'] as const
const ARM_HAND_L_ALT = ['ParamArmLB03'] as const
const ARM_HAND_R_ALT = ['ParamArmRB03'] as const

const ANY_ARM_L = [...ARM_UPPER_L, ...ARM_UPPER_L_ALT] as const
const ANY_ARM_R = [...ARM_UPPER_R, ...ARM_UPPER_R_ALT] as const

export const ACTION_DEFS = {
  happy: {
    label: 'Feliz',
    duration: 3.0,
    fadeIn: 0.5,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamEyeLSmile', 'ParamCheek', 'ParamTere', 'ParamMouthOpenY', 'ParamA']],
    curves: [
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.4, 12],
          [0.8, -8],
          [1.2, 10],
          [1.6, -6],
          [2.2, 4],
          [3.0, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.6, -8],
          [1.4, 8],
          [2.2, -3],
          [3.0, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.4, 6],
          [0.8, -3],
          [1.2, 6],
          [1.6, -2],
          [2.2, 3],
          [3.0, 0],
        ],
      },
      {
        param: BODY_ROLL,
        keys: [
          [0, 0],
          [0.7, -5],
          [1.5, 5],
          [2.3, -2],
          [3.0, 0],
        ],
      },

      {
        param: EYE_SMILE_L,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.4, 1],
          [3.0, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.4, 1],
          [3.0, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.3, -1],
          [2.4, -1],
          [3.0, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.3, -1],
          [2.4, -1],
          [3.0, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.3, 0.6],
          [2.4, 0.6],
          [3.0, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.3, 0.6],
          [2.4, 0.6],
          [3.0, 0],
        ],
      },
      {
        param: CHEEK,
        keys: [
          [0, 0],
          [0.3, 0.8],
          [2.4, 0.8],
          [3.0, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.35, 0.8],
          [0.9, 0.4],
          [1.3, 0.7],
          [2.0, 0.5],
          [2.6, 0],
          [3.0, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.4, 0.8],
          [0.9, 0.2],
          [1.3, 0.7],
          [2.0, 0.3],
          [2.6, 0],
          [3.0, 0],
        ],
      },
    ],
  },

  wink: {
    label: 'Piscadinha',
    duration: 2.0,
    fadeIn: 0.5,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamEyeROpen']],
    curves: [
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.3, -1],
          [1.2, -1],
          [1.5, 0],
          [2.0, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.3, 1],
          [1.2, 1],
          [1.5, 0],
          [2.0, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.3, -0.3],
          [1.2, -0.3],
          [1.6, 0],
          [2.0, 0],
        ],
      },
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.35, 6],
          [1.3, 6],
          [2.0, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.35, -12],
          [1.3, -12],
          [2.0, 0],
        ],
      },
      {
        param: BODY_ROLL,
        keys: [
          [0, 0],
          [0.4, -4],
          [1.3, -4],
          [2.0, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.35, 0.5],
          [1.2, 0.3],
          [1.7, 0],
          [2.0, 0],
        ],
      },
    ],
  },

  nod: {
    label: 'Concordar',
    duration: 2.2,
    fadeIn: 0.4,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamAngleY']],
    curves: [
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.35, -22],
          [0.7, -4],
          [1.0, -18],
          [1.5, 0],
          [2.2, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.4, -3],
          [1.1, -2],
          [1.6, 0],
          [2.2, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.35, -0.45],
          [0.7, -0.1],
          [1.0, -0.4],
          [1.5, 0],
          [2.2, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.35, -0.45],
          [0.7, -0.1],
          [1.0, -0.4],
          [1.5, 0],
          [2.2, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.35, 0.3],
          [1.5, 0],
          [2.2, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.35, 0.3],
          [1.5, 0],
          [2.2, 0],
        ],
      },
    ],
  },

  shakehead: {
    label: 'Discordar',
    duration: 2.4,
    fadeIn: 0.4,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamAngleX']],
    curves: [
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.3, -24],
          [0.7, 20],
          [1.1, -16],
          [1.5, 10],
          [1.9, 0],
          [2.4, 0],
        ],
      },
      {
        param: BODY_YAW,
        keys: [
          [0, 0],
          [0.35, -4],
          [0.75, 4],
          [1.15, -3],
          [1.55, 2],
          [2.0, 0],
          [2.4, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.3, -0.3],
          [1.5, -0.3],
          [1.9, 0],
          [2.4, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.3, -0.3],
          [1.5, -0.3],
          [1.9, 0],
          [2.4, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.3, -0.5],
          [1.6, -0.5],
          [2.1, 0],
          [2.4, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.3, -0.5],
          [1.6, -0.5],
          [2.1, 0],
          [2.4, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.3, -1.8],
          [1.6, -1.8],
          [2.1, 0],
          [2.4, 0],
        ],
      },
    ],
  },

  thinking: {
    label: 'Pensando',
    duration: 4.0,
    fadeIn: 0.5,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamAngleZ', 'ParamAngleX'], ['ParamEyeBallX']],
    curves: [
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.8, -12],
          [3.2, -12],
          [4.0, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.8, 6],
          [3.2, 6],
          [4.0, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.8, 14],
          [3.2, 14],
          [4.0, 0],
        ],
      },
      {
        param: EYE_BALL_X,
        keys: [
          [0, 0],
          [0.7, -0.8],
          [2.0, -0.8],
          [2.4, -0.5],
          [3.2, -0.8],
          [4.0, 0],
        ],
      },
      {
        param: EYE_BALL_Y,
        keys: [
          [0, 0],
          [0.7, 0.7],
          [3.2, 0.7],
          [4.0, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.8, -0.25],
          [3.2, -0.25],
          [4.0, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.8, -0.25],
          [3.2, -0.25],
          [4.0, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.8, -0.3],
          [3.2, -0.3],
          [4.0, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.8, -0.3],
          [3.2, -0.3],
          [4.0, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.8, -0.7],
          [3.2, -0.7],
          [4.0, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.8, -0.7],
          [3.2, -0.7],
          [4.0, 0],
        ],
      },
      {
        param: BROW_ANGLE_L,
        keys: [
          [0, 0],
          [0.8, -0.5],
          [3.2, -0.5],
          [4.0, 0],
        ],
      },
      {
        param: BROW_ANGLE_R,
        keys: [
          [0, 0],
          [0.8, -0.5],
          [3.2, -0.5],
          [4.0, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.8, -2.2],
          [3.2, -2.2],
          [4.0, 0],
        ],
      },
      {
        param: BODY_ROLL,
        keys: [
          [0, 0],
          [0.9, 6],
          [3.2, 6],
          [4.0, 0],
        ],
      },
    ],
  },

  surprised: {
    label: 'Surpresa',
    duration: 2.5,
    fadeIn: 0.2,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamEyeLOpen']],
    curves: [
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.15, 0.2],
          [1.4, 0.2],
          [2.0, 0],
          [2.5, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.15, 0.2],
          [1.4, 0.2],
          [2.0, 0],
          [2.5, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.15, 1],
          [1.4, 1],
          [2.0, 0],
          [2.5, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.15, 1],
          [1.4, 1],
          [2.0, 0],
          [2.5, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.15, 0.3],
          [1.4, 0.3],
          [2.0, 0],
          [2.5, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.15, 0.3],
          [1.4, 0.3],
          [2.0, 0],
          [2.5, 0],
        ],
      },

      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.15, -2.5],
          [1.4, -2.5],
          [2.1, 0],
          [2.5, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.15, 0.9],
          [1.4, 0.8],
          [2.1, 0],
          [2.5, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.15, 18],
          [0.5, 12],
          [1.4, 12],
          [2.1, 0],
          [2.5, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.18, 8],
          [1.4, 6],
          [2.2, 0],
          [2.5, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.15, 1],
          [1.4, 0.8],
          [2.1, 0],
          [2.5, 0],
        ],
      },
    ],
  },

  shy: {
    label: 'Envergonhada',
    duration: 4.0,
    fadeIn: 0.5,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamCheek', 'ParamTere', 'ParamEyeBallY']],
    curves: [
      {
        param: CHEEK,
        keys: [
          [0, 0],
          [0.6, 1],
          [3.4, 1],
          [4.0, 0],
        ],
      },
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.8, 10],
          [2.0, 6],
          [3.3, 9],
          [4.0, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.7, -14],
          [3.3, -12],
          [4.0, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.9, 7],
          [2.2, 4],
          [3.3, 7],
          [4.0, 0],
        ],
      },
      {
        param: EYE_BALL_X,
        keys: [
          [0, 0],
          [0.7, 0.6],
          [2.0, 0.4],
          [3.3, 0.6],
          [4.0, 0],
        ],
      },
      {
        param: EYE_BALL_Y,
        keys: [
          [0, 0],
          [0.7, -0.6],
          [3.3, -0.5],
          [4.0, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.7, -0.35],
          [3.3, -0.35],
          [4.0, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.7, -0.35],
          [3.3, -0.35],
          [4.0, 0],
        ],
      },
      {
        param: EYE_SMILE_L,
        keys: [
          [0, 0],
          [0.7, 0.5],
          [3.3, 0.5],
          [4.0, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.7, 0.5],
          [3.3, 0.5],
          [4.0, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.7, -0.2],
          [3.3, -0.2],
          [4.0, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.7, -0.2],
          [3.3, -0.2],
          [4.0, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.7, -0.6],
          [3.3, -0.6],
          [4.0, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.7, -0.6],
          [3.3, -0.6],
          [4.0, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.7, -0.7],
          [3.3, -0.7],
          [4.0, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.7, 0.15],
          [3.3, 0.1],
          [4.0, 0],
        ],
      },
      {
        param: BODY_ROLL,
        keys: [
          [0, 0],
          [1.0, -6],
          [2.2, -3],
          [3.4, -6],
          [4.0, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.7, 0.5],
          [3.3, 0.5],
          [4.0, 0],
        ],
      },
    ],
  },

  angry: {
    label: 'Irritada',
    duration: 2.6,
    fadeIn: 0.3,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamBrowLY', 'ParamMouthForm']],
    curves: [
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.25, -0.8],
          [2.0, -0.8],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.25, -0.8],
          [2.0, -0.8],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.25, -0.9],
          [2.0, -0.9],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.25, -0.9],
          [2.0, -0.9],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.25, -0.2],
          [2.0, -0.2],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.25, -0.2],
          [2.0, -0.2],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.25, -1.6],
          [2.0, -1.6],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.3, 0.35],
          [1.1, 0.2],
          [2.0, 0.3],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.2, -10],
          [0.6, -6],
          [2.0, -8],
          [2.4, 0],
          [2.6, 0],
        ],
      },
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.25, -8],
          [0.7, 6],
          [1.2, -4],
          [2.0, 0],
          [2.6, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.25, -5],
          [2.0, -4],
          [2.4, 0],
          [2.6, 0],
        ],
      },
    ],
  },

  sad: {
    label: 'Triste',
    duration: 3.4,
    fadeIn: 0.5,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamAngleY', 'ParamEyeBallY']],
    curves: [
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.8, -16],
          [2.8, -14],
          [3.4, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.9, -6],
          [2.8, -5],
          [3.4, 0],
        ],
      },
      {
        param: EYE_BALL_Y,
        keys: [
          [0, 0],
          [0.8, -0.7],
          [2.8, -0.6],
          [3.4, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.8, -0.4],
          [2.8, -0.4],
          [3.4, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.8, -0.4],
          [2.8, -0.4],
          [3.4, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.8, -0.5],
          [2.8, -0.5],
          [3.4, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.8, -0.5],
          [2.8, -0.5],
          [3.4, 0],
        ],
      },
      {
        param: BROW_FORM_L,
        keys: [
          [0, 0],
          [0.8, -0.8],
          [2.8, -0.8],
          [3.4, 0],
        ],
      },
      {
        param: BROW_FORM_R,
        keys: [
          [0, 0],
          [0.8, -0.8],
          [2.8, -0.8],
          [3.4, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.8, -1.4],
          [2.8, -1.4],
          [3.4, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.9, -4],
          [2.8, -3],
          [3.4, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.8, -0.5],
          [2.8, -0.5],
          [3.4, 0],
        ],
      },
    ],
  },

  wave: {
    label: 'Acenar',
    duration: 2.8,
    fadeIn: 0.3,
    fadeOut: 0.5,
    requiresAnyOf: [ANY_ARM_R],
    curves: [
      {
        param: ARM_UPPER_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [2.2, 0.62],
          [2.8, 0],
        ],
      },
      {
        param: ARM_UPPER_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [2.2, 0.62],
          [2.8, 0],
        ],
      },

      {
        param: ARM_FORE_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.7, 0.55],
          [2.2, 0.5],
          [2.8, 0],
        ],
      },
      {
        param: ARM_FORE_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.7, 0.55],
          [2.2, 0.5],
          [2.8, 0],
        ],
      },
      {
        param: ARM_HAND_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.8, 0.75],
          [1.1, -0.6],
          [1.4, 0.75],
          [1.7, -0.6],
          [2.0, 0.7],
          [2.4, 0],
          [2.8, 0],
        ],
      },
      {
        param: ARM_HAND_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.8, 0.75],
          [1.1, -0.6],
          [1.4, 0.75],
          [1.7, -0.6],
          [2.0, 0.7],
          [2.4, 0],
          [2.8, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.6, 7],
          [1.8, 5],
          [2.8, 0],
        ],
      },
      {
        param: BODY_ROLL,
        keys: [
          [0, 0],
          [0.6, 4],
          [1.8, 3],
          [2.8, 0],
        ],
      },
      {
        param: EYE_SMILE_L,
        keys: [
          [0, 0],
          [0.5, 1],
          [2.2, 1],
          [2.8, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.5, 1],
          [2.2, 1],
          [2.8, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.5, -1],
          [2.2, -1],
          [2.8, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.5, -1],
          [2.2, -1],
          [2.8, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.6, 0.4],
          [1.8, 0.3],
          [2.8, 0],
        ],
      },
    ],
  },

  shrug: {
    label: 'Dar de ombros',
    duration: 2.4,
    fadeIn: 0.3,
    fadeOut: 0.5,
    requiresAnyOf: [[...SHOULDER, ...ANY_ARM_L, ...ANY_ARM_R]],
    curves: [
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.45, 1],
          [1.5, 0.9],
          [2.4, 0],
        ],
      },
      {
        param: ARM_UPPER_L,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },
      {
        param: ARM_UPPER_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },
      {
        param: ARM_UPPER_L_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },
      {
        param: ARM_UPPER_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },

      {
        param: ARM_HAND_L,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.6, 0.65],
          [1.5, 0.6],
          [2.4, 0],
        ],
      },
      {
        param: ARM_HAND_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.6, 0.65],
          [1.5, 0.6],
          [2.4, 0],
        ],
      },
      {
        param: ARM_HAND_L_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.6, 0.65],
          [1.5, 0.6],
          [2.4, 0],
        ],
      },
      {
        param: ARM_HAND_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.6, 0.65],
          [1.5, 0.6],
          [2.4, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.5, 10],
          [1.5, 9],
          [2.4, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.5, 6],
          [1.5, 5],
          [2.4, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.45, 0.5],
          [1.5, 0.45],
          [2.4, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.5, -0.4],
          [1.5, -0.35],
          [2.4, 0],
        ],
      },
    ],
  },

  cheer: {
    label: 'Comemorar',
    duration: 2.6,
    fadeIn: 0.2,
    fadeOut: 0.5,
    requiresAnyOf: [[...ANY_ARM_L, ...ANY_ARM_R]],
    curves: [
      {
        param: ARM_UPPER_L,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.35, 0.9],
          [0.9, 0.35],
          [1.4, 0.75],
          [2.0, 0.2],
          [2.6, 0],
        ],
      },
      {
        param: ARM_UPPER_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.35, 0.9],
          [0.9, 0.35],
          [1.4, 0.75],
          [2.0, 0.2],
          [2.6, 0],
        ],
      },
      {
        param: ARM_UPPER_L_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.35, 0.9],
          [0.9, 0.35],
          [1.4, 0.75],
          [2.0, 0.2],
          [2.6, 0],
        ],
      },
      {
        param: ARM_UPPER_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.35, 0.9],
          [0.9, 0.35],
          [1.4, 0.75],
          [2.0, 0.2],
          [2.6, 0],
        ],
      },
      {
        param: ARM_FORE_L,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [1.0, 0.25],
          [1.55, 0.6],
          [2.1, 0.15],
          [2.6, 0],
        ],
      },
      {
        param: ARM_FORE_R,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [1.0, 0.25],
          [1.55, 0.6],
          [2.1, 0.15],
          [2.6, 0],
        ],
      },
      {
        param: ARM_FORE_L_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [1.0, 0.25],
          [1.55, 0.6],
          [2.1, 0.15],
          [2.6, 0],
        ],
      },
      {
        param: ARM_FORE_R_ALT,
        unit: 'normalized',
        keys: [
          [0, 0],
          [0.5, 0.7],
          [1.0, 0.25],
          [1.55, 0.6],
          [2.1, 0.15],
          [2.6, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.35, 0.7],
          [0.9, 0.3],
          [1.4, 0.6],
          [2.6, 0],
        ],
      },
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.35, 14],
          [0.9, 2],
          [1.4, 11],
          [2.0, 2],
          [2.6, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.35, 7],
          [0.9, 1],
          [1.4, 6],
          [2.0, 1],
          [2.6, 0],
        ],
      },
      {
        param: EYE_SMILE_L,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.1, 1],
          [2.6, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.1, 1],
          [2.6, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.3, -1],
          [2.1, -1],
          [2.6, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.3, -1],
          [2.1, -1],
          [2.6, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.35, 0.9],
          [0.9, 0.4],
          [1.4, 0.8],
          [2.6, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.35, 0.6],
          [2.1, 0.5],
          [2.6, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.35, 0.6],
          [2.1, 0.5],
          [2.6, 0],
        ],
      },
    ],
  },

  laugh: {
    label: 'Gargalhada',
    duration: 2.4,
    fadeIn: 0.3,
    fadeOut: 0.5,
    requiresAnyOf: [
      ['ParamMouthOpenY', 'ParamA'],
      ['ParamEyeLSmile', 'ParamEyeLOpen'],
    ],
    curves: [
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.3, 14],
          [0.6, 8],
          [0.9, 13],
          [1.2, 7],
          [1.5, 11],
          [2.0, 3],
          [2.4, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [0.3, 5],
          [0.6, 2],
          [0.9, 5],
          [1.2, 2],
          [1.5, 4],
          [2.4, 0],
        ],
      },
      {
        param: SHOULDER,
        keys: [
          [0, 0],
          [0.3, 0.8],
          [0.6, 0.3],
          [0.9, 0.8],
          [1.2, 0.3],
          [1.5, 0.6],
          [2.4, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.3, -0.7],
          [1.8, -0.7],
          [2.4, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.3, -0.7],
          [1.8, -0.7],
          [2.4, 0],
        ],
      },
      {
        param: EYE_SMILE_L,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.0, 1],
          [2.4, 0],
        ],
      },
      {
        param: EYE_SMILE_R,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.0, 1],
          [2.4, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [0.3, 0.9],
          [0.6, 0.4],
          [0.9, 0.9],
          [1.2, 0.4],
          [1.5, 0.8],
          [2.0, 0.2],
          [2.4, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.3, 1],
          [2.0, 1],
          [2.4, 0],
        ],
      },
    ],
  },

  pout: {
    label: 'Bico',
    duration: 2.2,
    fadeIn: 0.4,
    fadeOut: 0.5,
    requiresAnyOf: [['ParamMouthForm'], ['ParamBrowLAngle', 'ParamBrowLForm']],
    curves: [
      {
        param: HEAD_YAW,
        keys: [
          [0, 0],
          [0.4, -14],
          [1.7, -14],
          [2.2, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [0.4, 8],
          [1.7, 8],
          [2.2, 0],
        ],
      },
      {
        param: EYE_BALL_X,
        keys: [
          [0, 0],
          [0.4, -0.8],
          [1.7, -0.8],
          [2.2, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.4, -0.4],
          [1.7, -0.4],
          [2.2, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.4, -0.4],
          [1.7, -0.4],
          [2.2, 0],
        ],
      },
      {
        param: BROW_ANGLE_L,
        keys: [
          [0, 0],
          [0.4, -0.6],
          [1.7, -0.6],
          [2.2, 0],
        ],
      },
      {
        param: BROW_ANGLE_R,
        keys: [
          [0, 0],
          [0.4, 0.6],
          [1.7, 0.6],
          [2.2, 0],
        ],
      },
      {
        param: MOUTH_FORM,
        keys: [
          [0, 0],
          [0.4, -1],
          [1.7, -1],
          [2.2, 0],
        ],
      },
      {
        param: CHEEK,
        keys: [
          [0, 0],
          [0.5, 0.6],
          [1.7, 0.6],
          [2.2, 0],
        ],
      },
    ],
  },

  sleepy: {
    label: 'Sono',
    duration: 3.2,
    fadeIn: 0.6,
    fadeOut: 0.8,
    requiresAnyOf: [['ParamEyeLOpen', 'ParamEyeROpen']],
    curves: [
      {
        param: HEAD_PITCH,
        keys: [
          [0, 0],
          [0.9, -10],
          [1.3, -6],
          [2.2, -14],
          [2.6, -9],
          [3.2, 0],
        ],
      },
      {
        param: HEAD_ROLL,
        keys: [
          [0, 0],
          [1.2, 6],
          [2.4, 9],
          [3.2, 0],
        ],
      },
      {
        param: BODY_PITCH,
        keys: [
          [0, 0],
          [1.0, -3],
          [2.3, -5],
          [3.2, 0],
        ],
      },
      {
        param: EYE_OPEN_L,
        keys: [
          [0, 0],
          [0.8, -0.6],
          [1.4, -0.4],
          [2.2, -0.85],
          [3.2, 0],
        ],
      },
      {
        param: EYE_OPEN_R,
        keys: [
          [0, 0],
          [0.8, -0.6],
          [1.4, -0.4],
          [2.2, -0.85],
          [3.2, 0],
        ],
      },
      {
        param: EYE_BALL_Y,
        keys: [
          [0, 0],
          [0.9, -0.5],
          [2.2, -0.7],
          [3.2, 0],
        ],
      },
      {
        param: BROW_Y_L,
        keys: [
          [0, 0],
          [0.9, -0.3],
          [2.2, -0.4],
          [3.2, 0],
        ],
      },
      {
        param: BROW_Y_R,
        keys: [
          [0, 0],
          [0.9, -0.3],
          [2.2, -0.4],
          [3.2, 0],
        ],
      },
      {
        param: MOUTH_OPEN,
        keys: [
          [0, 0],
          [1.6, 0.05],
          [2.4, 0.35],
          [2.9, 0.1],
          [3.2, 0],
        ],
      },
    ],
  },
} as const satisfies Record<string, ActionDef>

export type ActionName = keyof typeof ACTION_DEFS

export const ACTION_NAMES = Object.keys(ACTION_DEFS) as ActionName[]

export const CONVENTIONAL_RANGES: Record<string, readonly [number, number]> = {
  ParamAngleX: [-30, 30],
  ParamAngleY: [-30, 30],
  ParamAngleZ: [-30, 30],
  ParamBodyAngleX: [-10, 10],
  ParamBodyAngleY: [-10, 10],
  ParamBodyAngleZ: [-10, 10],
  ParamEyeLOpen: [0, 2],
  ParamEyeROpen: [0, 2],
  ParamEyeLSmile: [0, 1],
  ParamEyeRSmile: [0, 1],
  ParamEyeBallX: [-1, 1],
  ParamEyeBallY: [-1, 1],
  ParamBrowLY: [-1, 1],
  ParamBrowRY: [-1, 1],
  ParamBrowLForm: [-1, 1],
  ParamBrowRForm: [-1, 1],
  ParamBrowLAngle: [-1, 1],
  ParamBrowRAngle: [-1, 1],
  ParamMouthForm: [-2, 1],
  ParamMouthOpenY: [0, 1],
  ParamA: [0, 1],
  ParamCheek: [0, 1],
  ParamTere: [0, 1],
  ParamShoulder: [-1, 1],
}

export const CONVENTIONAL_BASES: Record<string, number> = {
  ParamEyeLOpen: 1,
  ParamEyeROpen: 1,
}

export const UNOBSERVED_ALLOWLIST: Record<string, readonly string[]> = {
  Mao: ['ParamCheek'],
}
