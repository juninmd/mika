/**
 * Anime outfits authored as extra meshes inside haru.vrm, hidden unless a VRM look reveals its own
 * `Outfit_<Look>_*` pieces (see HARU_OUTFITS in vrm-look.ts). Torso and sleeves stay Haru's: the
 * body mesh under them was cut away, so each outfit retints them instead of replacing them.
 */
import { curvedPanel, garmentBack, garmentFront, torsoSection } from './haru-garments.mjs'
import {
  apron,
  bow,
  crownFrill,
  flaredSkirt,
  hairOutline,
  insetEdge,
  line,
  line3,
  petticoat,
  ribbonTails,
  waistband,
} from './haru-outfit-shapes.mjs'

export function addOutfits({ T, add, material, clothMap, skirtWeights, hair }) {
  const kit = {
    T,
    add,
    skirtWeights,
    head: hairOutline(hair),
    m: {
      navy: material('Outfit_Navy', ['#2c3a66', '#161d3a'], { texture: clothMap, outline: 0.0011 }),
      white: material('Outfit_White', ['#fbfaff', '#c4c3d6'], { outline: 0.0006 }),
      red: material('Outfit_Red', ['#e0364f', '#8c1d34'], { outline: 0.0006, rim: 0.3 }),
      black: material('Outfit_Black', ['#2b2735', '#121019'], { texture: clothMap, outline: 0.0011 }),
      pink: material('Outfit_Pink', ['#ff7fb2', '#c9447e'], { outline: 0.0006, rim: 0.3 }),
      pleatWhite: material('Outfit_PleatWhite', ['#fdf8fc', '#d6c0da'], { texture: clothMap, outline: 0.0011 }),
    },
  }
  seifuku(kit)
  maid(kit)
  idol(kit)
}

function seifuku({ T, add, m, skirtWeights }) {
  const skirt = { length: 0.72, flare: 0.5, pleats: 24, depth: 0.012 }
  add('Outfit_Seifuku_Skirt', flaredSkirt(T, skirt), m.navy, 'hips', skirtWeights)
  // Sailor collar: a V on each breast and the square flap on the back, both edged by two stripes.
  for (const side of [-1, 1]) {
    const front = [
      [side * 0.03, 1.333],
      [side * 0.106, 1.272],
      [0, 1.172],
    ]
    add('Outfit_Seifuku_Collar', curvedPanel(T, front, 0.009), m.navy, 'chest')
    for (const inset of [0.006, 0.012]) {
      const edge = insetEdge(front[1], front[2], front[0], inset).map(([x, y]) => [side * Math.max(0, side * x), y])
      line(T, add, 'Outfit_Seifuku_Stripe', edge, garmentFront, 0.0026, m.white)
    }
  }
  const flap = [
    [-0.035, 1.336],
    [0.035, 1.336],
    [0.09, 1.288],
    [0.104, 1.165],
    [-0.104, 1.165],
    [-0.09, 1.288],
  ]
  add('Outfit_Seifuku_Collar', curvedPanel(T, flap, 0.009, garmentBack), m.navy, 'chest')
  for (const inset of [0.008, 0.014]) {
    const edge = [
      [-0.09 + inset, 1.288],
      [-0.104 + inset, 1.165 + inset],
      [0.104 - inset, 1.165 + inset],
      [0.09 - inset, 1.288],
    ]
    line(T, add, 'Outfit_Seifuku_Stripe', edge, garmentBack, 0.0026, m.white)
  }
  const knot = garmentFront(0, 1.178, 0.02)
  bow(T, add, 'Outfit_Seifuku_Bow', m.red, 'chest', knot, 0.034)
  ribbonTails(T, add, 'Outfit_Seifuku_Bow', m.red, knot, 0.014)
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

function idol({ T, add, m, skirtWeights, head }) {
  const skirt = { length: 0.64, flare: 0.58, pleats: 20, depth: 0.013 }
  add('Outfit_Idol_Skirt', flaredSkirt(T, skirt), m.pleatWhite, 'hips', skirtWeights)
  add('Outfit_Idol_Petticoat', petticoat(T, skirt, 0.03), m.pink, 'hips', skirtWeights)
  bow(T, add, 'Outfit_Idol_Bow', m.pink, 'chest', garmentFront(0, 1.268, 0.022), 0.036)
  ribbonTails(T, add, 'Outfit_Idol_Bow', m.pink, garmentFront(0, 1.262, 0.024), 0.015)
  const [x, y, z] = head.at(Math.PI * 0.26, 0.012)
  bow(T, add, 'Outfit_Idol_HeadBow', m.pink, 'head', [x, y, z + 0.02], 0.042, -0.45)
}
