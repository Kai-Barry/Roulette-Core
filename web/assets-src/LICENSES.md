# Asset Licence Ledger

Every shipped asset in `web/public/assets/` MUST have a row here (Stage 2 blocking gate,
`plan/asset-pipeline-3d-upgrade.md`). Raw sources archived under `web/assets-src/`
(git-ignored except this file and hash records).

Sources used this phase (P1): **Quaternius** (quaternius.com) — CC0 1.0 Universal,
confirmed via `License.txt` shipped inside the author's Google Drive distribution
("CC0 1.0 Universal / Public Domain Dedication — https://creativecommons.org/publicdomain/zero/1.0/").
Optional credit requested by author: "Background characters by Quaternius — https://www.patreon.com/quaternius"
(recorded here as the project's standing credit line).

| Shipped file | Source pack | Derived from (raw) | Licence | Source URL | Fetched | Raw sha256 |
|---|---|---|---|---|---|---|
| `models/croupier_sitting.glb` | Quaternius — Background Posed Humans, Female Poses/OBJ | `assets-src/quaternius/Female_Sitting.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/backgroundposedhumans.html (Drive obj id `1tsH40mlqGXeI7igey4i_gMDKx2wvTGRn`) | 2026-09-17 | `b9488206…cb5f8` (obj) / `16e1e3b…c6f53` (mtl) |
| `models/prop_stool.glb` | Quaternius — Furniture Kit, OBJ | `assets-src/quaternius/furn_Stool.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/furniture.html (Drive obj id `1uzubn5H3Xd12eARmEXOb5tm_yLR1KW4y`) | 2026-09-17 | see `/tmp/hashes.txt` capture below |
| `models/prop_chair.glb` | Quaternius — Furniture Kit, OBJ | `assets-src/quaternius/furn_Chair.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/furniture.html (Drive obj id `1bp5KnhzdMgYuImiUGhEC89uEq7XRPILi`) | 2026-09-17 | — |
| `models/prop_table.glb` | Quaternius — Furniture Kit, OBJ | `assets-src/quaternius/furn_Table.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/furniture.html (Drive obj id `1HG79qIVqVocnaxauPW0jGjDS4qnk8m1x`) | 2026-09-17 | — |
| `models/prop_lamp.glb` | Quaternius — Furniture Kit, OBJ | `assets-src/quaternius/furn_Lamp.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/furniture.html (Drive obj id `16mkhq_piZAvZABzcxFDzsBjbpKm_-7l4`) | 2026-09-17 | — |
| `models/prop_vase.glb` | Quaternius — Furniture Kit, OBJ | `assets-src/quaternius/furn_Vase.obj/.mtl` | CC0 1.0 | https://quaternius.com/packs/furniture.html (Drive obj id `1N80g5IutK0-pPDYdiLj-4jB7GJdquHtN`) | 2026-09-17 | — |
| `models/prop_chair.glb` etc. | converted+palette-baked | tools/assets/bake_palette.py | — | — | — | shipped sha256s recorded in `web/public/assets/manifest.json` |

Spare/unused raw (kept for future phases, not shipped):
- `assets-src/posedhumans/Female_Standing.blend` (Drive id `1KSZM5pjOAn6olj1n8fjA7KQRprrS0RrV`, CC0)
- `assets-src/quaternius/Female_Standing_Hips.obj/.mtl`, `Female_Standing_CoveringEyes.obj/.mtl` (CC0)

## Shipped GLB sha256 (manifest reference)

```
52c9d6645b227bfe01cc530a5feac685790d7019ea4ef04c6bab6be00c381273  croupier_sitting.glb
e3bb70cebabbfdf8a444c133a4db27ec8b4900d689c25b1e113cbe2fb9fa7327  prop_chair.glb
be29e5a53f3ea9b4024ebcca9f4e397a99b87cc4a5bec8f32acc22941ddfb8e7  prop_lamp.glb
8785da9298b7a656091b17c6deb080e6bd0505b457b20cba1a7b96e73ee716e8  prop_stool.glb
34e287029bd6536341857f9274a5c54aa6c8dead43151d2234d7b20af59e6f30  prop_table.glb
323d960a4e4ec087866726b6f3d16f19aa117ceb07068d98e5b719e9a2d64664  prop_vase.glb
```

## Conversion notes (Stage 3+4 record)

- OBJ→GLB: `obj2gltf --binary` (Cesium, MIT), node 22.
- Palette bake: `tools/assets/bake_palette.py` — material-name remap into game palette
  (mahogany `#3a2317`, dark mahogany `#230e0c`, brass `#c9a227`, croupier charcoal-blue),
  metallic=0/roughness=1 (Lambert-grade), PBR maps stripped, 5-bit channel quantization
  pre-baked so the PS1 post pass does not double-shift colors.
- Vertex snapping to 1/64 grid: **deferred to integration stage** (must be applied in the
  same pass that normalizes scale/orientation against the live scene, else it's blind).
- Poly counts (well under R3 budgets): stool 106 v, chair 108 v, table 74 v, lamp 56 v,
  vase 104 v, croupier 680 v (~1.3–2.6k tris).
- Scale note: Quaternius models import ~1:1 meters; croupier seated height ≈ 2.5 world
  units in OBJ — integration step must normalize to scene (felt y=0, croupier seat y≈1.4).