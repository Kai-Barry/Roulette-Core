# Pipeline: Free 3D Models → "Roulette of the Damned" Visual Upgrade

**Status:** PROPOSAL — nothing in this document is implemented. No code, asset, or
dependency has been added to the repo. This is the plan of record for user approval.

**Goal:** Replace the procedural placeholder props (boxes, capsules, torus) with free,
licensable 3D models **without breaking the established PS1/PS2 casino-horror aesthetic**
(blueprint §1.4: chunky low-poly, dithered 5-bit rendering, mahogany + brass + felt green
palette, crimson accents, oppressive candle-lit atmosphere).

---

## 0. The cohesion thesis (read first)

The game's look is currently defined by three things, in order of strength:

1. **The PS1 post pass** (`PS1Shader.ts`, TASK-023) — 320×180 internal render, pixelation,
   4×4 Bayer dither, 5-bit color quantization. **Anything rendered through this pass
   automatically inherits the aesthetic.** This is our single strongest cohesion tool.
2. **Palette discipline** — mahogany/brass/felt-green + crimson accents, gold UI. 
   Assets that carry their own baked-in colors must be re-tinted to this palette or rejected.
3. **Poly/texture budget** — PS1-era chunkiness. High-detail PBR models would *fight*
   the look even through the pixelation pass (texture shimmer, aliasing noise, wrong
   silhouette granularity).

Therefore the pipeline's job is **not** "make models look good" — it is
**"make downloaded models pass through the PS1 lens and the palette filter so they
look like they were always native to this game."** Every stage below exists to serve that.

**Cohesion rules (the "make it native" test):**
- R1 — Every asset passes through the PS1 post pass. No asset is rendered clean.
- R2 — Texture resolution ceiling: ≤ 128×128 px (hero assets ≤ 256), `NearestFilter`,
  pre-quantized to 5-bit levels during conversion so the post pass doesn't shift them.
- R3 — Poly budget: felt/props ≤ 1,500 tris; wheel & ball ≤ 4,000 (it's the hero);
  croupier/enemy figures ≤ 3,000; environment/silhouettes ≤ 2,000.
- R4 — Color: baked textures re-tinted into the game palette (mahogany `#3a2317`,
  brass `#c9a227`, felt `#0d3b2a`, crimson `#d81838`, bone `#e8e0c8`) or vertex-color
  overridden. No pastel/clean-modern colors survive conversion.
- R5 — Lighting: all materials must be Lambert-grade (no PBR metalness/roughness maps
  survive — PS1 era had no specular maps on most props). Baked AO allowed if subtle.
- R6 — Procedural fallbacks stay wired: if an asset fails to load, the scene falls back
  to the current primitive version. The game must never break in headless/`?no3d=1` mode.

---

## 1. Sources (all verified reachable 2026-09-14; CC0-first)

| Source | Licence | Formats | What we'd pull |
|---|---|---|---|
| Kenney (kenney.nl) | **CC0** | GLTF/GLB, OBJ | Casino chips, furniture, props packs; entire thematic kits |
| Quaternius (quaternius.com) | **CC0** | GLTF/GLB | Furniture kits, low-poly animals/figures (croupier bases) |
| Poly Haven (polyhaven.com) | **CC0** (assets, HDRIs) | GLTF/FBX, HDR | Candles, brass props, wood, HDRs for baked lighting study |
| AmbientCG (ambientcg.com) | **CC0** | PBR texture sets | Wood grain / felt / brass textures — downressed to R2 budget |
| Poly Pizza (poly.pizza) | per-asset (many CC0/CC-BY) | GLB | Casino search results — **only** assets whose licence field reads CC0/CC-BY with attribution recorded; API needs key, so web crawl only if needed |

**Rule:** nothing enters `web/public/assets/` without a row in the licence ledger (§4).
Poly Pizza is last resort; Kenney + Quaternius + AmbientCG alone cover every need in
§3 and are licence-trivial.

---

## 2. Asset inventory — what we'd actually replace

Grounded in the current code (files named, so implementation later is mechanical):

| # | Current placeholder | Lives in | Candidate source | Priority |
|---|---|---|---|---|
| A1 | Bell (torus + knob), brass | `dressing.ts` | Kenney furniture/props or Poly Haven candle-bell | **HIGH** — player-facing interaction point (spin) |
| A2 | Chip stacks (instanced cylinders) | `dressing.ts` | Kenney casino/chip pack; per-denom tint via instance color | **HIGH** — always on screen |
| A3 | Croupiers (capsule + sphere silhouettes) | `dressing.ts` | Quaternius figure kit + palette re-tint to §11.6 croupier palettes | **HIGH** — creepiest placeholder; biggest atmosphere win |
| A4 | Felt surface | `dressing.ts` | AmbientCG felt/velvet, 128px, re-tinted `#0d3b2a` | MED — texture only, geometry stays |
| A5 | Wheel ball (plain sphere) | `WheelVisual.ts` | Stays primitive (ivory sphere) or Poly Haven billiard-ball scan, decimated | LOW — a sphere is already period-correct |
| A6 | Room shell / pillars / candle sconces | — (currently absent) | Quaternius architecture kit + Poly Haven candles | MED — adds oppression without touching gameplay |
| A7 | Enemy table props (cards, hourglass, skull) | — (§10.7 intent area) | Kenney boardgame/props packs | MED |
| A8 | Dust motes | `dressing.ts` Points | Stays procedural (correct already) | NONE |

Explicitly **not** touching: wheel geometry (blueprint-mandated, canvas-textured from
engine state — REQ-002), felt sector layout, pick targets, cameras, fx rings.

---

## 3. Pipeline stages (design only)

```
[1 SOURCE] → [2 LEDGER] → [3 CONVERT] → [4 COHESION BAKE] → [5 MANIFEST] → [6 INTEGRATE] → [7 VALIDATE]
   raw GLB      licence      decimate,      retexture,          hash,        swap-in          GL-shot
   fetch       entry        downres,      retint, PS1-        manifest      behind           A/B diff
               (BLOCK)      transform     pre-quantize        (BLOCK)       fallbacks        (BLOCK)
```

### Stage 1 — Acquisition (`tools/assets/fetch.py`, new)
- Input: a checked-in `plan/asset-manifest-wishlist.yml` listing source, page URL, asset ID.
- Fetches raw archives to `web/assets-src/<source>/<asset-id>/` (git-ignored; never shipped).
- Verifies HTTP 200 + non-empty zip; extracts GLB/GLTF + textures into a `raw/` subdir.
- **Never** auto-follows links off the whitelist. Sources are hardcoded domain allowlist
  (kenney.nl, quaternius.com, polyhaven.com, ambientcg.com, poly.pizza).

### Stage 2 — Licence ledger (BLOCKING gate)
- `web/assets-src/LICENSES.md` — one row per asset: source, asset id, licence name, licence
  URL, download URL, fetch date, sha256 of the archive.
- CI/plan-level check: `web/public/assets/**` files must all appear in the ledger or the
  build fails. No ledger row = asset never ships. This protects us from CC-BY-ND or
  "non-commercial only" traps sneaking in.

### Stage 3 — Conversion (`tools/assets/convert.mjs`, new; gltf-transform CLI)
- Decimate to poly budget (R3) with weld + prune; drop PBR extensions not in the
  Lambert whitelist (R5) — `KHR_materials_unlit` → MeshLambertMaterial mapping, drop
  `KHR_materials_specular`, normal maps (PS1!), clearcoat, sheen, transmission.
- Textures: downscale to R2 ceiling, resize with nearest neighbor to preserve crunch,
  **then** pre-quantize channels to 5-bit (32 levels) so the shader pass doesn't double-
  dither baked colors.
- Draco/meshopt compression optional (meshopt preferred — smaller, decode is cheap).
- Unit normalize: scale to the game's meters (wheel rim = r 4 at y 2 per WheelVisual).
- Output: `web/public/assets/models/<asset-id>.glb` + `.ktx2`-free (we use plain PNG at
  ≤128px — KTX2/basis adds a loader dependency; avoid until needed).

### Stage 4 — Cohesion bake (the "make it native" pass)
- Palette re-tint (R4): LUT-based remap of baked texture colors toward game palette
  anchors, done as an offline image step (Pillow in the fetch script or a small node
  canvas script). Applied *after* downres so banding matches the PS1 look.
- Vertex snapping: quantize vertex positions to a 1/64 grid — classic PS1 jitter look,
  makes hand-modeled and kit-bashed assets share the same "imprecision" as our
  procedural geometry.
- Flip Y+Z check + one-authored screenshot per asset on a turntable against a felt
  background — archived to `design-review/asset-candidates/` for your review (this doc's
  §6 shows the same gallery workflow used for cards).

### Stage 5 — Deterministic manifest (BLOCKING gate, serves REQ-006)
- `web/public/assets/manifest.json` — asset id → sha256, tri count, texture sizes, licence
  ledger ref.
- Runtime `AssetLibrary` (proposed new module, NOT implemented) refuses an asset whose
  hash mismatches — replays stay byte-identical (REQ-006), and corrupted files fail
  loudly instead of silently swapping geometry.

### Stage 6 — Integration points (mapped to existing modules, zero gameplay surface)
| Swap point | Module today | Change shape |
|---|---|---|
| Bell mesh | `dressing.ts` line ~63 (torus) | `AssetLibrary.get('bell')` → clone GLB scene → same `userData.pick`, same name |
| Chip stacks | `dressing.ts` instanced mesh | GLB chip becomes InstancedMesh source geometry; denom color via instance color |
| Croupiers | `dressing.ts` capsule+sphere | GLB figure; palette swap per §11.6 |
| Felt texture | `dressing.ts` Lambert color | Lambert + 128px re-tinted texture map |
| Room shell | new `dressing2.ts` group | Additive-only; no pick targets; behind the fog near-plane |
| Wheel ball | `WheelVisual.ts` sphere | Optional swap, default stays primitive |
| **Fallback wiring** | every swap point | try/catch around load → keep procedural object if GLB absent/invalid |

Hard constraint: **pick targets, command flow (PAT-001), camera rig (§11.3), and engine
state projection (REQ-002) do not move.** Assets are visual grafts on the same skeleton.
`?no3d=1` keeps working (no GLB fetch at all), `?legacy=1` unaffected, and
`window.__RT` harness keeps its exact command surface (REQ-005).

### Stage 7 — Validation (BLOCKING gate, reuses existing harness)
- The GL screenshot tour (`/tmp/shots/gltour.mjs` from the card-gallery work) is promoted
  into `web/test/visual/` — before/after A/B pairs for each swap point at 320×180 native
  res.
- Cohesion checks, automated per shot: (a) distinct-color count within ±40% of current
  baseline (~232–286 colors — if a model explodes the palette it fails); (b) palette-
  distance: dominant hue clusters must sit within the §1.4 palette wheel; (c) 5-bit
  quantization histogram — post-pass must not introduce banding cliffs.
- Poly budget check in the manifest lint (Stage 5).
- Play a full legal-command tour (menu → draft → floor → combat → spin) driving only via
  `window.__RT` (REQ-005) with seed fixed (REQ-006) — screenshots must match pre/post
  behavior identically for non-visual harness reads.

---

## 4. Directory layout (proposed, nothing created yet)

```
web/assets-src/            # raw downloads + licence ledger (git-ignored except LICENSES.md)
  LICENSES.md
  kenney/<id>/raw/…
  quaternius/<id>/raw/…
web/public/assets/
  manifest.json
  models/*.glb            # converted, ≤ R3 budget
  textures/*.png          # ≤128px, palette-baked
web/tools/ or tools/assets/
  fetch.py                # whitelist-downloader + ledger writer
  convert.mjs             # gltf-transform pipeline (decimate → detexture → retint → snap)
  bake_palette.py         # LUT retint (Pillow)
  snapshot.mjs            # per-asset turntable → design-review/asset-candidates/
web/test/visual/          # promoted GL tour + cohesion color checks
```

## 5. Work order & rough effort (if approved later)

| Phase | Deliverable | Effort | Risk |
|---|---|---|---|
| P1 | fetch + ledger + convert for **bell, chips, croupiers** (A1–A3) | ~1 session | low |
| P2 | felt texture + room shell + candle props (A4, A6, A7) | ~1 session | low-med |
| P3 | cohesion tuning (palette LUT, vertex snap rate) + full GL A/B gallery | ~½ session | med (taste) |
| P4 | wheel ball (A5) + polish, final A/B set into design-review/ | ~½ session | low |

Every phase ships something reviewable as an A/B screenshot set; rollback is per-asset
(delete manifest row → procedural fallback takes over automatically).

## 6. What approval means

If you approve: I'd start with P1 exactly as scoped — fetch three assets, run the ledger,
convert, and show you the A/B screenshot gallery before touching `dressing.ts`. Nothing
in the game changes until you've seen the candidates side by side and picked.

If you want changes to the direction (different sources, no palette-bake, more/less
aggressive retinting), edit this doc or tell me and I'll revise before any code.