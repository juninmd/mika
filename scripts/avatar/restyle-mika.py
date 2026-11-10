#!/usr/bin/env python3
"""
Redesenha o visual do Mika VRM direto no glTF, sem round-trip pelo Blender.

Por que não exportar do Blender: o exportador glTF do Blender descarta as extensões VRM (humanoide,
expressões, spring bones, MToon, constraints) — o modelo deixaria de ser um VRM. Aqui só trocamos PNGs
embutidos e fatores de material; geometria, esqueleto, física e blendshapes ficam byte a byte iguais.

  python3 scripts/avatar/restyle-mika.py --source mika-original.vrm --out apps/desktop/src/renderer/public/vrm/mika/mika.vrm

`--source` é o VRM ORIGINAL (o recolor parte do gradiente roxo/turquesa; sobre um arquivo já recolorido o
resultado sairia errado). O original está no histórico: `git show d16b130:apps/desktop/src/renderer/public/vrm/mika/mika.vrm`.

Mudanças:
  • cabelo: preto na raiz, azul nas pontas (Hair_00 e HairBack_00 + sombra/rim do material)
  • jaqueta: rosa pastel com estampa de pixel-art (corações, estrelas, controle) e mangas lilás
"""
import argparse
import io
import json
import struct
import sys

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

GLB_MAGIC = 0x46546C67
CHUNK_JSON = 0x4E4F534A
CHUNK_BIN = 0x004E4942


def read_glb(path):
    data = open(path, "rb").read()
    magic, _version, _length = struct.unpack_from("<III", data, 0)
    if magic != GLB_MAGIC:
        sys.exit(f"{path}: não é um GLB/VRM")
    json_len, json_type = struct.unpack_from("<II", data, 12)
    assert json_type == CHUNK_JSON
    gltf = json.loads(data[20 : 20 + json_len])
    bin_off = 20 + json_len
    bin_len, bin_type = struct.unpack_from("<II", data, bin_off)
    assert bin_type == CHUNK_BIN
    return gltf, bytearray(data[bin_off + 8 : bin_off + 8 + bin_len])


def write_glb(path, gltf, binbuf):
    while len(binbuf) % 4:
        binbuf.append(0)
    gltf["buffers"][0]["byteLength"] = len(binbuf)
    js = json.dumps(gltf, separators=(",", ":")).encode()
    js += b" " * (-len(js) % 4)
    total = 12 + 8 + len(js) + 8 + len(binbuf)
    with open(path, "wb") as f:
        f.write(struct.pack("<III", GLB_MAGIC, 2, total))
        f.write(struct.pack("<II", len(js), CHUNK_JSON) + js)
        f.write(struct.pack("<II", len(binbuf), CHUNK_BIN) + bytes(binbuf))


def image_bytes(gltf, binbuf, image_index):
    view = gltf["bufferViews"][gltf["images"][image_index]["bufferView"]]
    start = view.get("byteOffset", 0)
    return bytes(binbuf[start : start + view["byteLength"]])


def png(array):
    out = io.BytesIO()
    Image.fromarray(array.astype(np.uint8), "RGBA").save(out, "PNG", optimize=True)
    return out.getvalue()


def replace_image(gltf, binbuf, image_index, data):
    """Acrescenta o PNG novo ao BIN e aponta a imagem para ele (o antigo vira lixo no fim; é pequeno)."""
    while len(binbuf) % 4:
        binbuf.append(0)
    gltf["bufferViews"].append({"buffer": 0, "byteOffset": len(binbuf), "byteLength": len(data)})
    binbuf.extend(data)
    gltf["images"][image_index]["bufferView"] = len(gltf["bufferViews"]) - 1


def add_texture(gltf, binbuf, data, name):
    gltf["images"].append({"name": name, "mimeType": "image/png", "bufferView": 0})
    replace_image(gltf, binbuf, len(gltf["images"]) - 1, data)
    gltf["textures"].append({"sampler": 0, "source": len(gltf["images"]) - 1})
    return len(gltf["textures"]) - 1


def material(gltf, name):
    return next(m for m in gltf["materials"] if m["name"] == name)


def smoothstep(edge0, edge1, x):
    t = np.clip((x - edge0) / (edge1 - edge0), 0, 1)
    return t * t * (3 - 2 * t)


# ───────────────────────────── cabelo ─────────────────────────────

ROOT = np.array([13, 15, 30], dtype=np.float32)  # preto azulado: preto puro some no contorno
TIP = np.array([34, 102, 255], dtype=np.float32)
TIP_END = np.array([96, 178, 255], dtype=np.float32)  # a ponta clareia, como tinta que desbota
SHEEN = np.array([70, 88, 150], dtype=np.float32)


def box_smooth(values, size):
    kernel = np.ones(size, dtype=np.float32) / size
    pad = size // 2
    padded = np.pad(values, (pad, pad), mode="edge")
    return np.convolve(padded, kernel, mode="valid")


def recolor_hair(rgba):
    """Preserva o relevo (fios e faixa de brilho) do gradiente original e troca só a cor."""
    rgba = rgba.astype(np.float32)
    height = rgba.shape[0]
    lum = (rgba[..., 0] * 0.299 + rgba[..., 1] * 0.587 + rgba[..., 2] * 0.114) / 255.0
    row = lum.mean(axis=1)
    row_s = box_smooth(row, 9)
    ratio = np.clip(lum / np.maximum(row_s[:, None], 1e-3), 0.75, 1.35)
    baseline = box_smooth(row, 201)
    band = np.clip((row_s - baseline) / 0.08, 0, 1)[:, None, None]

    v = ((np.arange(height) + 0.5) / height)[:, None, None]
    tip = smoothstep(0.64, 0.96, v)
    end = smoothstep(0.86, 1.0, v)
    base = ROOT * (1 - tip) + TIP * tip
    base = base * (1 - end) + TIP_END * end
    out = base * ratio[..., None] + SHEEN * band * (1 - tip)
    out = np.clip(out, 0, 255)
    result = rgba.copy()
    result[..., :3] = out
    return result


def restyle_hair(gltf, binbuf):
    for mat_name in ("Hair_00_HAIR", "HairBack_00_HAIR"):
        mat = material(gltf, mat_name)
        texture = mat["pbrMetallicRoughness"]["baseColorTexture"]["index"]
        image = gltf["textures"][texture]["source"]
        source = np.array(Image.open(io.BytesIO(image_bytes(gltf, binbuf, image))).convert("RGBA"))
        replace_image(gltf, binbuf, image, png(recolor_hair(source)))
        mtoon = mat["extensions"]["VRMC_materials_mtoon"]
        # A sombra multiplica a mesma textura: um tom azulado claro mantém o cabelo preto vivo, não carvão.
        mtoon["shadeColorFactor"] = [0.55, 0.62, 0.95]
        mtoon["parametricRimColorFactor"] = [0.08, 0.14, 0.34]


# ───────────────────────────── jaqueta ─────────────────────────────

SIZE = 2048
PINK = (248, 201, 220)
PINK_SHADE = (240, 178, 205)
HOT = (255, 92, 150)
WHITE = (255, 250, 253)
MINT = (120, 224, 200)
SKY = (120, 170, 255)
LILAC = (222, 208, 245)
LILAC_SHADE = (205, 188, 236)
NAVY = (40, 48, 110)

# Ícones 8x8 de pixel-art: 1 = pixel aceso.
HEART = [
    "01100110",
    "11111111",
    "11111111",
    "11111111",
    "01111110",
    "00111100",
    "00011000",
    "00000000",
]
STAR = [
    "00011000",
    "00011000",
    "11111111",
    "01111110",
    "00111100",
    "01111110",
    "01100110",
    "00000000",
]
SPARKLE = [
    "00010000",
    "00010000",
    "00111000",
    "11111110",
    "00111000",
    "00010000",
    "00010000",
    "00000000",
]
CAT = [
    "10000001",
    "11000011",
    "11111111",
    "10111101",
    "11111111",
    "11011011",
    "01111110",
    "00000000",
]
ICONS = {"heart": HEART, "star": STAR, "sparkle": SPARKLE, "cat": CAT}


def stamp(draw, icon, x, y, cell, color, squash=1.0):
    """`squash` < 1 encurta o ícone na vertical: a UV da jaqueta rende ~0,58x mais textura por metro em
    V do que em U, então um ícone quadrado na imagem sai esticado para cima no corpo (medido no mesh)."""
    height = max(1, round(cell * squash))
    for row, line in enumerate(icon):
        for col, bit in enumerate(line):
            if bit == "1":
                draw.rectangle([x + col * cell, y + row * height, x + (col + 1) * cell - 1, y + (row + 1) * height - 1], fill=color)


def pixel_print(base, shade, icons, palette, cell=8, step=140, seed=7, squash=1.0):
    """Estampa all-over: malha diagonal de ícones. UV da jaqueta é uma projeção plana, então um padrão
    uniforme é o único desenho que não depende de onde cada peça cai na textura."""
    img = Image.new("RGBA", (SIZE, SIZE), base + (255,))
    # Trama de ponto de malha por baixo, bem sutil: dá tecido sem competir com a estampa.
    px = np.array(img, dtype=np.float32)
    yy, xx = np.mgrid[0:SIZE, 0:SIZE]
    weave = np.sin((xx + yy) * 2 * np.pi / 14) * np.sin((xx - yy) * 2 * np.pi / 14)
    px[..., :3] *= (0.965 + 0.035 * (0.5 + 0.5 * weave))[..., None]
    img = Image.fromarray(np.clip(px, 0, 255).astype(np.uint8), "RGBA")
    draw = ImageDraw.Draw(img)
    rng = np.random.default_rng(seed)
    names = list(icons)
    row_step = max(1, round(step * squash))
    for gy, y in enumerate(range(24, SIZE - 8 * cell, row_step)):
        for gx, x in enumerate(range(24 + (step // 2) * (gy % 2), SIZE - 8 * cell, step)):
            name = names[int(rng.integers(len(names)))]
            color = palette[int(rng.integers(len(palette)))]
            # sombra deslocada de 1 pixel: dá o relevo do bordado
            stamp(draw, ICONS[name], x + cell // 2, y + cell // 2, cell, shade, squash)
            stamp(draw, ICONS[name], x, y, cell, color, squash)
    return np.array(img)


def restyle_jacket(gltf, binbuf):
    jacket = pixel_print(PINK, PINK_SHADE, ["heart", "star", "sparkle", "cat", "heart"], [WHITE, HOT, SKY, WHITE], seed=7, squash=0.58)
    sleeves = pixel_print(LILAC, LILAC_SHADE, ["star", "heart", "sparkle"], [WHITE, WHITE, (255, 170, 205)], cell=6, step=150, seed=11)
    for mat_name, art, shade in (
        ("Renewal_Jacket", jacket, [0.86, 0.66, 0.78]),
        ("Renewal_Sleeves", sleeves, [0.78, 0.7, 0.9]),
    ):
        mat = material(gltf, mat_name)
        tex = add_texture(gltf, binbuf, png(art), f"{mat_name}_print")
        mat["pbrMetallicRoughness"]["baseColorFactor"] = [1, 1, 1, 1]
        mat["pbrMetallicRoughness"]["baseColorTexture"] = {"index": tex}
        mtoon = mat["extensions"]["VRMC_materials_mtoon"]
        mtoon["shadeColorFactor"] = shade
        mtoon["shadeMultiplyTexture"] = {"index": tex}
        # Contorno rosado em vez do azul-marinho quase preto: traço duro sobre rosa-pastel pesa demais.
        mtoon["outlineColorFactor"] = [0.42, 0.18, 0.3] if mat_name == "Renewal_Jacket" else [0.34, 0.26, 0.48]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--source", required=True)
    parser.add_argument("--out", required=True)
    args = parser.parse_args()
    gltf, binbuf = read_glb(args.source)
    restyle_hair(gltf, binbuf)
    restyle_jacket(gltf, binbuf)
    write_glb(args.out, gltf, binbuf)
    print(f"ok → {args.out}")


if __name__ == "__main__":
    main()
