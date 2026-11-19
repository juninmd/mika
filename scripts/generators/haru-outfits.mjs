/**
 * Anime outfits authored as extra meshes inside haru.vrm, hidden unless a VRM look reveals its own
 * `Outfit_<Look>_*` pieces (see HARU_OUTFITS in vrm-look.ts). Torso and sleeves stay Haru's: the
 * body mesh under them was cut away, so each outfit retints them instead of replacing them.
 */
import { curvedPanel, garmentFront, torsoSection } from './haru-garments.mjs'
import {
  apron,
  bow,
  crownFrill,
  flaredSkirt,
  hairOutline,
  line,
  line3,
  petticoat,
  waistband,
} from './haru-outfit-shapes.mjs'

export function addOutfits({ T, add, material, clothMap, skirtWeights, hair }) {
  const kit = {
    T,
    add,
    skirtWeights,
    head: hairOutline(hair),
    m: {
      white: material('Outfit_White', ['#fbfaff', '#c4c3d6'], { outline: 0.0006 }),
      black: material('Outfit_Black', ['#2b2735', '#121019'], { texture: clothMap, outline: 0.0011 }),
    },
  }
  maid(kit)
}

function maid({ T, add, m, skirtWeights, head }) {
  const skirt = { length: 0.7, flare: 0.52, pleats: 0, depth: 0.014 }
  add('Outfit_Maid_Skirt', flaredSkirt(T, skirt), m.black, 'hips', skirtWeights)
  add('Outfit_Maid_Petticoat', petticoat(T, skirt, 0.024), m.white, 'hips', skirtWeights)
  add('Outfit_Maid_Apron', apron(T, skirt), m.white, 'hips', skirtWeights)
  const bib = [
    [-0.058, 1.205],
    [0.058, 1.205],
    [0.043, 1.0],
    [0.064, 0.862],
    [-0.064, 0.862],
    [-0.043, 1.0],
  ]
  add('Outfit_Maid_Bib', curvedPanel(T, bib, 0.01), m.white, 'chest')
  for (const side of [-1, 1]) {
    line(
      T,
      add,
      'Outfit_Maid_Frill',
      [
        [side * 0.058, 1.205],
        [side * 0.046, 1.08],
        [side * 0.043, 1.0],
      ],
      garmentFront,
      0.0032,
      m.white,
    )
    line(
      T,
      add,
      'Outfit_Maid_Strap',
      [
        [side * 0.052, 1.203],
        [side * 0.07, 1.265],
        [side * 0.082, 1.305],
      ],
      garmentFront,
      0.0042,
      m.white,
    )
  }
  line(
    T,
    add,
    'Outfit_Maid_Frill',
    [
      [-0.058, 1.205],
      [0, 1.2],
      [0.058, 1.205],
    ],
    garmentFront,
    0.0032,
    m.white,
  )
  waistband(T, add, 'Outfit_Maid_Waist', 0.985, m.white)
  const [, , back] = torsoSection(0.985)
  bow(T, add, 'Outfit_Maid_WaistBow', m.white, 'chest', [0, 0.985, -back - 0.014], 0.04)
  // Katyusha: a lace crown that follows the hair outline ear to ear, over a black band.
  const arc = Array.from({ length: 41 }, (_, i) => Math.PI * (0.13 + (0.74 * i) / 40))
  add('Outfit_Maid_HeadFrill', crownFrill(T, head, arc, 0.02), m.white, 'head')
  line3(
    T,
    add,
    'Outfit_Maid_HeadBand',
    arc.map((angle) => head.at(angle, 0.004)),
    0.0034,
    m.black,
    'head',
  )
}
