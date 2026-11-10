export const FPS = 30

export function snap(time: number): number {
  return Math.round((Math.round(time * FPS) / FPS) * 1000) / 1000
}

export function round3(value: number): number {
  return Math.round(value * 1000) / 1000
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max)
}

export interface BuiltCurve {
  Target: 'Parameter'
  Id: string
  Segments: number[]
}

export function buildCurve(paramId: string, keys: Array<[number, number]>): BuiltCurve {
  const snapped = keys.map(([time, value]) => [snap(time), round3(value)] as [number, number])
  const segments: number[] = [snapped[0][0], snapped[0][1]]
  for (let i = 0; i < snapped.length - 1; i++) {
    const [t0, v0] = snapped[i]
    const [t1, v1] = snapped[i + 1]
    const third = (t1 - t0) / 3
    segments.push(1, round3(t0 + third), v0, round3(t1 - third), v1, t1, v1)
  }
  return { Target: 'Parameter', Id: paramId, Segments: segments }
}

export interface MotionFile {
  Version: 3
  Meta: {
    Duration: number
    Fps: number
    Loop: false
    AreBeziersRestricted: true
    CurveCount: number
    TotalSegmentCount: number
    TotalPointCount: number
    UserDataCount: 0
    TotalUserDataSize: 0
  }
  Curves: BuiltCurve[]
}

export function buildMotion(duration: number, curves: BuiltCurve[]): MotionFile {
  let totalSegments = 0
  let totalPoints = 0
  for (const curve of curves) {
    const seg = curve.Segments
    totalPoints += 1
    let i = 2
    while (i < seg.length) {
      totalSegments += 1
      if (seg[i] === 1) {
        totalPoints += 3
        i += 7
      } else {
        totalPoints += 1
        i += 3
      }
    }
  }
  return {
    Version: 3,
    Meta: {
      Duration: snap(duration),
      Fps: FPS,

      Loop: false,
      AreBeziersRestricted: true,
      CurveCount: curves.length,
      TotalSegmentCount: totalSegments,
      TotalPointCount: totalPoints,
      UserDataCount: 0,
      TotalUserDataSize: 0,
    },
    Curves: curves,
  }
}
