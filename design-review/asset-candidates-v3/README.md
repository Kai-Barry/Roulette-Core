# Asset candidates — review round 3 (P1.6, better-quality sources)

**Context:** round-2 verdict was "the models themselves are technically badly made."
Geometry audit confirmed it: the Quaternius furniture kit renders clean (0–1
degenerate tris, correct normal splits) but the *silhouettes and forms are crude* —
that's the author, not our pipeline. Round 3 sources higher-quality authors:

| Source | Licence | Notes |
|---|---|---|
| Poly Pizza — Google Poly archival scans | CC-BY 3.0 (per-asset attribution) | professionally-crafted props, scan-quality forms |
| Poly Pizza — Polygonal Mind, Hunter Paramore, iPoly3D, CreativeTrio, jeremy | CC0 / CC-BY 3.0 (per-asset, recorded in ledger) | curated low-poly artists |
| **Kenney Furniture Kit** (116 models) | **CC0** | coherent style across a whole set; CDN UUIDs harvested from bundle page |

All 40 candidates rendered **through the actual in-game look**: legacy-light Lambert
(matches `dressing.ts`), PS1 post pass (320×180 internal, Bayer dither, 5-bit quant —
same math as `PS1Shader.ts`), palette-anchor cubes in frame. Geometry audit
(`deg=degenerate tris, split=hard-edge normal splits — the crisp-edge signal`):

## Review sheets (start here)

| Sheet | Contents |
|---|---|
| contact-01-tables.png | 9 tables incl. current prop_table for comparison |
| contact-02-chairs-stools.png | 10 seats incl. current chair/stool |
| contact-03-lamps.png | 7 lamps incl. current prop_lamp |
| contact-04-vases-decor.png | 7 decor items incl. current prop_vase |
| contact-05-figures.png | 6 humanoid candidates incl. current croupier |
| contact-06-casino-bits.png | poker chips, dice, radio, vintage TV (set-dressing) |

Full-size per-model turntables: `turntable-<id>.png` in this folder.

## Notes per candidate (geometry audit + tri budget R3)

- Tables: `table_hunterparamore` 788 tris (CC-BY, Google Poly era, strong forms);
  `kenney_table`/`kenney_roundtable` CC0 coherent kit; old prop_table = 144 tris.
- Chairs/stools: `kenney_loungechair` 3.3k tris reads great but over R3 budget —
  decimation would fix; `barstool`/`stoolbarsq` CC0 & in-budget; `stool_ipoly3d` 140 tris.
- Lamps: `lamp_google` 918 tris (UV-only normals — needs our Lambert conversion);
  `kenney_lamp*` CC0, 3 styles.
- Figures: `female_polygonalmind` 2474 tris (within croupier budget 3000, CC0, real
  character silhouette); `wizard_polygonalmind` 3416 (slightly over, decimable);
  `woman_quat` 2004 but same crude Quaternius look; `dame1930_franck` 22k tris
  (needs heavy decimation, scan-quality otherwise).
- Casino bits: chips + dice + vintage TV + radio are strong set-dressing upgrades.

## Verdict matrix — fill in and reply

| Slot | Candidates (best-first guess) | Verdict |
|---|---|---|
| table | hunterparamore / kenney_table / kenney_coffeetable / table_jeremy | |
| chairs | kenney_chair / kenney_chairrounded / kenney_loungechair | |
| stools | kenney_barstool / stool_ipoly3d / kenney_stoolbarsq | |
| lamps | kenney_lamproundtable / lamp_google / kenney_lamproundfloor | |
| vase/decor | vase_jarlanperez / vase_google / kenney_pottedplant | |
| croupier | female_polygonalmind / wizard_polygonalmind / (keep old) | |
| set-dressing | chips / dice / tvvintage / radio / books / pottedplant | |

CC-BY 3.0 items need an attribution line in `LICENSES.md` (fine for shipping);
CC0 items are attribution-free. Retints remain free via `bake_palette.py` remaps.

---

# Weathering pass (P1.7) — before/after for the 5 user picks

`contact-07-weathered.png`: left = plain, right = weathered (`<id>-w.glb`).
Per-pair pixel diffs through the PS1 pipeline (all five verified changed):
table 2243px / stool 3861px / lamp 4678px / vase 1886px / character 1750px.

What was done (all textures ≤128px, palette-anchored, 5-bit pre-quantized — R2 budget):

| Asset | Was | Now |
|---|---|---|
| table_round | flat `#171209`-ish factor | 128px weathered mahogany grain + soot blotches |
| stool | flat wood + flat 'Cushin' | same wood + desaturated teal cloth tint on cushion |
| lamp_google | clean 256px Google art | art kept, resized 128px, soot-blotch grime multiply + wax stains |
| vase_jarlanperez | flat orange/white/red factors | weathered terracotta: two-tone fade + rim soot + drip streaks |
| basicchar_yogoshimo | single flat white material | bone/mahogany cloth-patch texture with weave lines + grime |

Untextured meshes got box-projected UVs (dominant-normal-axis projection); textured
meshes (lamp, character) keep original UVs. All materials forced Lambert-grade
(metallic 0, roughness 1) to match the legacy-light pipeline. Files:
`web/public/assets/models-candidates/<id>-w.glb`.

**Corr. to earlier notes:** the GLB JSON chunk magic is `0x4E4F534A` ("JSON") —
the "JSGN `0x4E4E5347`" note in AGENTS.md was a transcription error; standard GLBs
were always correct. `bake_palette.py` also emits the standard JSON magic.
**Harness caveat:** Chromium caches GLB responses per URL — when re-rendering a
changed asset, cache-bust with `?v=N` in the gallery ASSETS map.