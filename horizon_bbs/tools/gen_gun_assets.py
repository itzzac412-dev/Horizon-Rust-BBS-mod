#!/usr/bin/env python3
"""Cut the gun sprite sheet into transparent PNGs, extruded 3D OBJ models and
the indexed-colour Rust sprite consts embedded in runtime.rs.

usage: gen_gun_assets.py SHEET.png
The sheet has a checkerboard baked into the pixels, so the background is
removed by colour (near-white/near-grey) rather than by alpha.
"""
import sys
from pathlib import Path
import numpy as np
from PIL import Image
from scipy import ndimage as ndi

ROOT = Path(__file__).resolve().parents[1]
# Sprite index (reading order, 0-based) -> gun. Other sprites are still exported.
PICK = {'pistol': 15, 'rifle': 8, 'sniper': 9, 'shotgun': 10}
DEPTH = 3          # voxel thickness of the 3D models
LENGTH = 0.9       # model length in blocks
HEX = '0123456789abcdef'


def split(sheet):
    rgb = Image.open(sheet).convert('RGB')
    a = np.array(rgb).astype(int)
    bg = (a.min(2) >= 222) & ((a.max(2) - a.min(2)) <= 12)
    fg = ndi.binary_opening(~bg, iterations=1)
    lab, n = ndi.label(ndi.binary_dilation(fg, iterations=3))
    found = []
    for i, s in enumerate(ndi.find_objects(lab)):
        mask = fg & (lab == i + 1)
        if mask.sum() > 150:
            found.append((s[0].start, s[1].start, mask))
    found.sort(key=lambda f: (f[0] // 60, f[1]))
    sprites = []
    for _, _, mask in found:
        ys, xs = np.where(mask)
        y0, y1, x0, x1 = ys.min(), ys.max() + 1, xs.min(), xs.max() + 1
        rgba = np.zeros((y1 - y0, x1 - x0, 4), np.uint8)
        rgba[..., :3] = np.array(rgb)[y0:y1, x0:x1]
        rgba[..., 3] = mask[y0:y1, x0:x1] * 255
        sprites.append(rgba)
    return sprites


def level(rgba):
    """Rotate a diagonal long-gun sprite so its barrel axis is horizontal (barrel left, stock right)."""
    ys, xs = np.where(rgba[..., 3] > 0)
    pts = np.stack([xs - xs.mean(), ys - ys.mean()])
    vals, vecs = np.linalg.eigh(np.cov(pts))
    vx, vy = vecs[:, np.argmax(vals)]
    if vx < 0:
        vx, vy = -vx, -vy
    angle = np.degrees(np.arctan2(vy, vx))
    img = Image.fromarray(rgba).rotate(angle, resample=Image.NEAREST, expand=True)
    out = np.array(img)
    ys, xs = np.where(out[..., 3] > 127)
    out = out[ys.min():ys.max() + 1, xs.min():xs.max() + 1].copy()
    out[..., 3] = (out[..., 3] > 127) * 255
    return out


def level_pistol(rgba):
    """The pistol is drawn with its slide pointing down-right at ~38 degrees; level it and face it left like the others."""
    img = Image.fromarray(rgba).rotate(38, resample=Image.NEAREST, expand=True).transpose(Image.FLIP_LEFT_RIGHT)
    out = np.array(img)
    ys, xs = np.where(out[..., 3] > 127)
    out = out[ys.min():ys.max() + 1, xs.min():xs.max() + 1].copy()
    out[..., 3] = (out[..., 3] > 127) * 255
    return out


def quantize(rgba):
    """-> (palette [(r,g,b)] up to 16, indices; -1 = transparent). Only opaque pixels pick the palette."""
    alpha = rgba[..., 3] > 0
    pix = rgba[alpha][:, :3].reshape(-1, 1, 3)
    img = Image.fromarray(pix).quantize(colors=16, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    pal = np.array(img.getpalette()[:48]).reshape(-1, 3)
    idx = np.full(alpha.shape, -1, dtype=int)
    idx[alpha] = np.array(img).reshape(-1).astype(int)
    return [tuple(int(c) for c in p) for p in pal], idx


def rust_const(name, rgba):
    pal, idx = quantize(rgba)
    h, w = idx.shape
    rows = [''.join('.' if v < 0 else HEX[v] for v in r) for r in idx]
    out = [f'        pub const {name}: Sprite = Sprite {{', f'            width: {w},', f'            height: {h},',
           '            palette: &[' + ', '.join('[%d, %d, %d]' % p for p in pal) + '],', '            rows: &[']
    out += [f'                "{r}",' for r in rows]
    out += ['            ],', '        };']
    return '\n'.join(out)


def write_obj(path, tex_name, rgba, depth=DEPTH):
    h, w = rgba.shape[:2]
    solid = rgba[..., 3] > 0
    s = LENGTH / w
    verts, uvs, faces = [], [], []

    def quad(pts, px, py):
        u, v = (px + 0.5) / w, 1 - (py + 0.5) / h
        uvs.append((u, v)); ui = len(uvs)
        base = len(verts) + 1
        verts.extend(pts)
        faces.append((base, base + 1, base + 2, base + 3, ui))

    def filled(x, y):
        return 0 <= x < w and 0 <= y < h and solid[y, x]

    for y in range(h):
        for x in range(w):
            if not solid[y, x]:
                continue
            x0, x1 = x * s, (x + 1) * s
            y0, y1 = (h - y - 1) * s, (h - y) * s
            z0, z1 = -depth * s / 2, depth * s / 2
            if not filled(x - 1, y): quad([(x0, y0, z1), (x0, y0, z0), (x0, y1, z0), (x0, y1, z1)], x, y)
            if not filled(x + 1, y): quad([(x1, y0, z0), (x1, y0, z1), (x1, y1, z1), (x1, y1, z0)], x, y)
            if not filled(x, y - 1): quad([(x0, y1, z1), (x1, y1, z1), (x1, y1, z0), (x0, y1, z0)], x, y)
            if not filled(x, y + 1): quad([(x0, y0, z0), (x1, y0, z0), (x1, y0, z1), (x0, y0, z1)], x, y)
            quad([(x0, y0, z1), (x1, y0, z1), (x1, y1, z1), (x0, y1, z1)], x, y)
            quad([(x1, y0, z0), (x0, y0, z0), (x0, y1, z0), (x1, y1, z0)], x, y)
    lines = [f'mtllib {path.stem}.mtl', 'o ' + path.stem]
    lines += ['v %.5f %.5f %.5f' % v for v in verts]
    lines += ['vt %.5f %.5f' % t for t in uvs]
    lines += ['usemtl gun']
    lines += ['f %d/%d %d/%d %d/%d %d/%d' % (a, u, b, u, c, u, d, u) for a, b, c, d, u in faces]
    path.write_text('\n'.join(lines) + '\n')
    path.with_suffix('.mtl').write_text(
        f'newmtl gun\nKd 1 1 1\nmap_Kd ../textures/{tex_name}\n')


def main(sheet):
    sprites = split(sheet)
    assert len(sprites) == 19, f'expected 19 guns, found {len(sprites)}'
    (ROOT / 'assets/textures').mkdir(parents=True, exist_ok=True)
    (ROOT / 'assets/models').mkdir(parents=True, exist_ok=True)
    names = {v: k for k, v in PICK.items()}
    consts = []
    for i, rgba in enumerate(sprites):
        name = names.get(i, f'sprite_{i:02d}')
        if name in ('rifle', 'sniper', 'shotgun'):
            rgba = level(rgba)
        elif name == 'pistol':
            rgba = level_pistol(rgba)
        tex = f'gun_{name}.png'
        Image.fromarray(rgba).save(ROOT / 'assets/textures' / tex)
        if i in names:
            write_obj(ROOT / 'assets/models' / f'gun_{name}.obj', tex, rgba)
            consts.append(rust_const(name.upper(), rgba))
    (ROOT / 'assets/sprites_generated.rs.txt').write_text('\n\n'.join(consts) + '\n')
    print('ok', len(sprites), 'sprites;', list(PICK))


if __name__ == '__main__':
    main(sys.argv[1])
