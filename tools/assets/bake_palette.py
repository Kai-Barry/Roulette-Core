#!/usr/bin/env python3
"""P1.3 palette-bake: remap GLB baseColorFactors into the game palette, strip PBR,
5-bit quantize (pre-bake for the PS1 pass). Written to tools/assets/bake_palette.py
per plan/asset-pipeline-3d-upgrade.md Stage 4."""
import struct, json, sys, os

PALETTE = [
    (0x3a/255,0x23/255,0x17/255),  # mahogany
    (0xc9/255,0xa2/255,0x27/255),  # brass
    (0x0d/255,0x3b/255,0x2a/255),  # felt
    (0xd8/255,0x18/255,0x38/255),  # crimson
    (0xe8/255,0xe0/255,0xc8/255),  # bone
    (0x1a/255,0x1a/255,0x24/255),  # charcoal (croupier suit)
]

def nearest(rgb):
    best, bi = None, 0
    for i, p in enumerate(PALETTE):
        d = sum((rgb[k]-p[k])**2 for k in range(3))
        if best is None or d < best:
            best, bi = d, i
    return PALETTE[bi]

def read_glb(b):
    magic, ver, total = struct.unpack('<III', b[:12])
    assert magic == 0x46546C67, "not a GLB"
    clen, ctype = struct.unpack('<II', b[12:20])
    assert ctype == 0x4E4F534A, "first chunk not JSON"  # 'JSON' little-endian
    j = json.loads(b[20:20+clen].decode('utf8'))
    bin_off = 20 + clen
    bin_chunk = None
    if bin_off < total:
        blen, btype = struct.unpack('<II', b[bin_off:bin_off+8])
        if btype == 0x004E4942:
            bin_chunk = b[bin_off+8:bin_off+8+blen]
    return j, bin_chunk

def bake(fname, remap):
    b = open(fname, 'rb').read()
    j, bin_chunk = read_glb(b)
    for m in j.get('materials', []):
        pbr = m.setdefault('pbrMetallicRoughness', {})
        c = list(pbr.get('baseColorFactor', [1,1,1,1]))
        name = (m.get('name') or '').lower()
        col = None
        for key, val in remap.items():
            if key != 'default' and key in name:
                col = val
                break
        if col is None:
            col = remap.get('default') or nearest(c[:3])
        c = [col[0], col[1], col[2], c[3] if len(c) > 3 else 1.0]
        pbr['baseColorFactor'] = [round(v*31)/31 for v in c]  # 5-bit pre-quantize
        pbr['metallicFactor'] = 0.0
        pbr['roughnessFactor'] = 1.0
        pbr.pop('baseColorTexture', None)
        m.pop('normalTexture', None)
        m.pop('occlusionTexture', None)
        m.pop('extensions', None)
    out = bytearray()
    out += struct.pack('<III', 0x46546C67, 2, 0)
    js = json.dumps(j, separators=(',',':')).encode('utf8')
    jpad = (4 - len(js) % 4) % 4
    out += struct.pack('<II', len(js)+jpad, 0x4E4F534A) + js + b' '*jpad
    if bin_chunk:
        bpad = (4 - len(bin_chunk) % 4) % 4
        out += struct.pack('<II', len(bin_chunk)+bpad, 0x004E4942) + bin_chunk + b'\0'*bpad
    out[8:12] = struct.pack('<I', len(out))
    open(fname, 'wb').write(bytes(out))
    print("baked", fname, "->", [(m.get('name'), m['pbrMetallicRoughness'].get('baseColorFactor')) for m in j.get('materials', [])])

if __name__ == '__main__':
    d = sys.argv[1] if len(sys.argv) > 1 else '.'
    croupier_remap = {
        'skin':  (0.596, 0.402, 0.251),
        'shirt': (0x2a/255, 0x2a/255, 0x38/255),
        'pants': (0x14/255, 0x14/255, 0x1c/255),
        'shoes': (0x22/255, 0x16/255, 0x10/255),
        'default': None,
    }
    wood_remap = {
        'darkwood': (0x23/255, 0x14/255, 0x0c/255),
        'wood':     (0x3a/255, 0x23/255, 0x17/255),
        'metal':    (0xc9/255, 0xa2/255, 0x27/255),
        'top':      (0xc9/255, 0xa2/255, 0x27/255),
        'default':  None,
    }
    bake(os.path.join(d, 'croupier_sitting.glb'), croupier_remap)
    for f in ['prop_stool.glb', 'prop_chair.glb', 'prop_table.glb', 'prop_lamp.glb', 'prop_vase.glb']:
        bake(os.path.join(d, f), wood_remap)