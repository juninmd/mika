export type Vec3 = [number, number, number] | number[]
export declare const smooth: (t: number, a: number, b: number) => number
export declare const gauss: (t: number, center: number, width: number) => number
export declare function warpNormal(warp: (p: number[]) => number[], position: Vec3, normal: Vec3): number[]
export declare function warpInPlace(
  warp: (p: number[]) => number[],
  positions: Float32Array,
  normals: Float32Array,
  filter?: (p: number[]) => boolean,
): void
