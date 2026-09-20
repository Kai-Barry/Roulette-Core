# Deploying Roulette.OS on a NAS

The game is a **fully static browser app**: the Rust engine compiles to WASM and
runs entirely in your browser. The NAS only serves files — no server-side logic,
no database, no runtime process. Verified end-to-end (menu → loadout → map →
combat) served from a plain `python3 -m http.server` with zero configuration.

## TL;DR

```bash
npm run build          # produces dist/
# copy the CONTENTS of dist/ to your NAS web root
```

Open `http://<nas-ip>:<port>/` and play.

A prebuilt artifact also exists at `dist/` (rebuild after engine changes).

---

## 1. Build the bundle

```bash
git clone https://github.com/Kai-Barry/Roulette-Core.git
cd Roulette-Core
npm ci
npm run build
```

- `npm run build` (Vite) emits **`dist/`** — ~1.8 MB total:
  - `index.html`, hashed JS bundle (three.js included, no CDN)
  - `assets/roulette_wasm_bg-*.wasm` — the engine
  - `assets/models/` + `assets/manifest.json` — decorative prop models
- The WASM pkgs (`crates/roulette-wasm/pkg`, `pkg-web`) are **prebuilt and
  committed**. You only need `wasm-pack` if you changed the Rust engine:

  ```bash
  cd crates/roulette-wasm
  wasm-pack build --target nodejs --out-dir pkg   --release   # tests/ai:check
  wasm-pack build --target web    --out-dir pkg-web --release  # frontend
  ```
  (Rebuild **both** after any engine change — see AGENTS.md.)

## 2. Upload to the NAS

Copy the **contents** of `dist/` (i.e. `index.html` at the top level) to the
NAS web root, not the `dist` folder itself. `index.html` must be reachable at
`/`.

```bash
# from your workstation, via SMB/NFS mount or scp:
tar -C dist -czf roulette-os-dist.tar.gz .
scp roulette-os-dist.tar.gz user@nas:/tmp/
ssh user@nas 'tar -xzf /tmp/roulette-os-dist.tar.gz -C /path/to/webroot'
```

Or just drag-and-drop the `dist` folder's contents with your NAS file manager.

### NAS-brand specifics

| NAS | Where |
|---|---|
| **Synology** | Control Panel → Web Services → enable "Web Station", set the document root; copy files to the `web` shared folder. Browse to `http://nas:80/` (or the configured port). |
| **QNAP** | Control Panel → Applications → Web Server → enable; copy files to `/share/Web/`. |
| **Unraid** | Use the "Nginx Webserver" / "Caddy Webserver" plugin, or the Docker option below with `/mnt/user/appdata/roulette` as the site root. |
| **TrueNAS SCALE** | Run the generic Docker option below as an app. |

### Generic option — Docker (works on every NAS)

```bash
docker run -d --name roulette-os --restart unless-stopped \
  -p 8080:80 \
  -v /path/to/dist:/usr/share/nginx/html:ro \
  nginx:alpine
```

Then open `http://<nas-ip>:8080/`. No config file needed; nginx:alpine serves
`.wasm` as `application/wasm` out of the box.

### Live-build Docker recipe (Unraid / SSH, no local node needed)

Build inside a throwaway node container and bind-mount `dist/` live —
container keeps running across rebuilds; updates are just a rebuild:

```bash
# once: get the repo + build dist (dist/ is gitignored, so build it yourself)
git clone https://github.com/Kai-Barry/Roulette-Core.git && cd Roulette-Core
git switch feat/roulette-cli-front-end   # branch with the 3D-visibility fix
docker run --rm -v "$PWD":/app -w /app node:24-slim sh -c "npm ci && npm run build"

# pick a FREE port (docker ps first! 8080/8081 are commonly taken by
# qBittorrent/Immich). Then serve:
docker run -d --name roulette-os --restart unless-stopped \
  -p 8084:80 \
  -v "$PWD/dist":/usr/share/nginx/html:ro \
  nginx:alpine

# verify you got the GAME, not another container's service:
curl -s http://localhost:8084/ | grep -ioc roulette   # must print >= 1

# later, to update (no container restart needed):
git pull
sudo rm -rf dist   # if a root-built dist is present
docker run --rm -v "$PWD":/app -w /app node:24-slim sh -c "npm ci && npm run build"
```

Gotchas hit in real deployments: a failed `docker run` still creates the
container — `docker rm -f roulette-os` before retrying; the smoke test must
grep for "roulette" (a port collision serves someone else's app and plain
`curl /` still returns 200 HTML).

## 3. Requirements & gotchas

- **Serve at domain/subdomain ROOT.** The built JS references `/assets/...` with
  absolute paths. `http://nas.local/` ✅ — `http://nas.local/roulette/` ❌
  (unless you rebuild with `vite build --base=/roulette/`, which is untested).
- **WASM MIME type.** Must be `application/wasm`. Modern nginx/Apache/Caddy/
  Python all ship this correctly. If your NAS gets it wrong the game still
  boots — wasm-bindgen falls back to a slower compile path with a console
  warning — but fix the mapping if you can.
- **Internet optional.** Google Fonts are the only external requests (cosmetic
  — the game degrades gracefully offline). Everything else is served locally.
- **Player saves are `localStorage`** — per browser, per device. Nothing is
  stored on the NAS; there is no cross-device sync.
- **HTTPS not required** on LAN. It only matters if you expose it publicly
  (use your NAS reverse proxy for TLS then).

## 4. Verify the deployment

1. `http://<nas-ip>:<port>/` shows the dark menu ("Roulette of the Damned").
2. Pick a descent → loadout store → map → enter a combat node.
3. Place a bet → SPIN. If the wheel resolves and HP/chips update, the WASM
   loaded fine (a black page + console `WASM` error ⇒ MIME-type problem).
4. Optional reproducibility check: `?seed=anything` gives the same run every
   time on any machine; plain URL resumes your autosave.

## 5. Updating

Rebuild and re-copy `dist/`; it is fully self-contained and replaces the old
files 1:1 (hashed filenames bust browser caches automatically). Saves survive
updates — they live in the player's browser, keyed by origin.