# Worldspring

Google-Maps-style procedural fantasy world for tabletop games: continent → battlemap zoom in the browser.
Public at github.com/Dun-John/worldspring (source, all rights reserved) and https://dun-john.github.io/worldspring/.
Local notes, not in the repo: `docs/authoring-handoff.md` (git-ignored: current state, backlog, the user's decisions;
read it at the start of a session) and the original plan (milestones M0–M9, all delivered) in
`~/.claude/plans/pasted-content-id-4ec8-help-me-lexical-raccoon.md`.

## Layout
- `crates/worldgen` — pure deterministic generator (Rust). No I/O, no threads inside generators,
  transcendentals only via `libm`, randomness only via `core::rng` hashing keyed by seed + coords.
- `crates/worldgen-wasm` — thin wasm-bindgen wrapper used by browser workers.
- `crates/mapd` — local agent server (127.0.0.1 only): MCP tools at `/mcp`, live sync with the app at `/ws`,
  worlds on disk in `worlds/`. See `docs/AGENT.md`.
- `app/src/ui/shell` — the app's frame: `layout.svelte.ts` (what is open: section World/Edit/Notes/Play and its tab,
  phone/short layouts as `data-layout` on `<html>`), `Dock` (right panel; on phones a bottom sheet over the tab bar),
  `TopBar` (☰ menu, search, breadcrumbs), `SectionBar`, `MapControls` (zoom, Layers, floor picker), `keymap.ts` (every
  keyboard shortcut, one listener; `?` shows them), `history.svelte.ts` (undo). In App, `go(section, tab)` opens a panel
  (asking before unsaved sketch strokes or a site design would be dropped) and `syncTool()` picks the map's pointer
  tool. Panel bodies take `peek` (only their tool strip). Look: tokens and `ws-` classes in `app.css`, icons in
  `ui/icons.ts`; no per-component breakpoints. Performance stats stay hidden unless asked for (` or the ☰ menu; any
  `?bench` run shows them).
- `app/` — Vite + TS + Svelte 5 + PixiJS v8 (WebGL2). `src/gen` workers, `src/render` map, `src/sync` mapd link,
  `src/editor` sketch mode (strokes in `WorldFile.sketch` steer T0: `crates/worldgen/src/t0/sketch.rs`) and scatter
  (battlemap objects put down or cleared by hand, uploaded sprites: `Edits.objects`/`cleared`/`sprites`, applied in
  `battlemap::apply_edits`; MCP in `crates/mapd/src/scatter.rs`),
  `src/play` play mode: the DM's tools plus a players' window (`app/player.html`, its own map and workers) kept in
  step over a BroadcastChannel by ops (`play/state.ts`); sessions per world hash in IndexedDB. DM-only: secret
  doors not yet found, hazards whose rules start "hidden" (traps, sinkholes), NPC markers, notes and plots.
  `src/editor/build.ts` + `ui/BuildPanel.svelte` draw buildings (`Created` kind `building`: one layout each,
  `town/sites.rs` `drawn`; placement check `agent::building_spot`; MCP in `crates/mapd/src/build.rs`).
  The same panel's Crossing mode (`src/editor/crossing.ts`) puts bridges, fords and ferries down bank to bank
  (`Edits.crossings`, `v:<id>`; drawn by `battlemap::hand_crossing` / `hand_decks`; MCP in `crates/mapd/src/crossings.rs`).
  `src/editor/site/` + `ui/DesignPanel.svelte` is the dungeon designer: an underground site (`u:`) copied into
  `Edits.designs` and built from there (`crates/worldgen/src/under/design.rs`: build, `check` with the vital
  rules, auto doors, text plans; MCP in `crates/mapd/src/design.rs`).
  `src/ui/Notebook.svelte` is the DM's notebook: NPCs and plot points (`Edits.npcs`/`plots`, MCP in
  `crates/mapd/src/notebook.rs`), authored, never generated.
  A city's sewers (generated per 480-ft section `w:<layout>:<sx>:<sy>`) are shown as one network
  (`render/Sewers.ts`: sections stream round the view, textures prepared in `underField.worker.ts`);
  a section's undercroft opens on its own. In play mode the sewer level is one location `w:<layout>`.
- `scripts/` — `build-wasm.mjs`, `det-wasm.mjs`, `bench.mjs`, `shot.mjs`, `publish.mjs`, `readme-media.mjs`.
- `README.md` — the repo's front page (using the site, running it, license), pictures in `docs/media/`;
  `THIRD_PARTY_NOTICES.md` — notices for code that follows others' work (Lucide icons).

## Commands
- First time: `npm --prefix app install` (also brings `wasm-opt`); Rust with the `wasm32-unknown-unknown` target and
  `cargo install wasm-bindgen-cli --version 0.2.129` (must match `Cargo.lock`).
- `npm run wasm` — build WASM into `app/src/gen/pkg` (required before `npm run dev`).
- `npm run dev` — dev server on :5173. `?seed=N` picks a world, `?bench=1` runs the perf fly-through
  (mountain, region, continent, then down into the biggest city), `?bench=play` the play-mode one (30 tokens,
  fog and line of sight in the biggest city), `?bench=sewer` panning through its sewers, `?bench=dungeon` the
  biggest ruin's dungeon (all its levels), `?bench=edit` scattering and clearing objects while panning a battlemap,
  `?bench=heavy` the stress bench (a world as generated, then loaded with ~6 MB of edits: 50 designed sites, 20k
  objects, 40 sprites, 2k NPCs…; edit latency and a battlemap pan, before and after; it refuses a world that has
  edits), `?gpu=1` adds GPU timer queries to any of them.
- `npm run dev -- -- --host` — the same, open to the LAN (Vite prints the Network URL; allow Node through the
  firewall once). A reverse proxy needs its hostname in `server.allowedHosts` (`app/vite.config.ts`); keep that
  line uncommitted (adding `host: true` there makes `--host` the default).
- `npm run mapd` — the agent server on 127.0.0.1:7777 (`-- --port N --dir worlds --app app/dist`); the app on
  localhost connects to it by itself (`?mapd=PORT` for another port, `?mapd=0` for none). Never expose it
  through the reverse proxy. Claude Code: `claude mcp add --transport http worldspring http://127.0.0.1:7777/mcp`.
- `npm run publish` — updates the live site: builds the app for GitHub Pages and force-pushes it as one commit to the
  repo's `gh-pages` branch (the `site` remote; commits use `git config site.email`, a GitHub noreply address). Every
  generator published stays on the site as its own build (`v<N>/`, listed in `versions.json`); a world from an older
  one asks to open as it was made or upgrade (`world/versions.ts`, App `settleVersion`).
- `cargo test --release -p worldgen` — the vital checks (`tests/vital.rs`).
- `npm run det` — WASM vs native byte-identical check.
- `npm run check` — svelte-check / TypeScript.
- `npm run bench` — perf gate in an unthrottled Chrome window (needs the dev server);
  `node scripts/bench.mjs "http://localhost:5173/?seed=1&bench=play"` for play mode.
- `node scripts/readme-media.mjs [url]` — the README's pictures into `docs/media/` from the dev server (default
  seed 1): the continent-to-inn zoom GIF (needs ffmpeg) and stills. Takes a few minutes; run again after visual changes.
- `node scripts/shot.mjs <prefix> [url] [views-json]` — screenshots at given camera views (visual checks;
  the automation Chrome tab is throttled in the background, so don't judge fps or rendering there).
  Echoes page exceptions and console errors; a view's optional `eval` expression is printed after the shot.
  `SHOT_PHONE=390,844,3` emulates a phone (touch, mobile layout); dev builds expose `window.__ui` (`go`, `select`,
  `shell`…) for `do` steps.
- `cargo run --release -p worldgen --example preview -- <seed> <out.png> [params-json] [sketch.json]` — T0 PNG +
  stats, including settlements, roads and handy screenshot locations (a king's road bridge, the longest switchbacks).
  With a sketch (`{"strokes":[…]}`), also the quick sketch preview (`<out>-quick.png`), conflicts and pinned settlements.
- `cargo run --release -p worldgen --example tilebench` — per-level tile generation timings.
- `cargo run --release -p worldgen --example citybench -- [seed] [radius]` — T0, metropolis layout and the
  battlemap chunks around it: timings plus output hashes (to prove a speed-up changes nothing).
- `cargo run --release -p worldgen --example seacheck -- <seed> <fx> <fy> [half-ft]` — wet/dry flips and
  height change between consecutive levels at fixed points (shoreline zoom consistency).
- `cargo run --release -p worldgen --example town -- <seed> <out-prefix> [index...]` — settlement layouts as
  SVG + stats/timings (default: metropolis, largest city, a town, a village).
- `cargo run --release -p worldgen --example roadcheck -- [seed...]` — roads, rivers and settlements QA (default seeds
  1 2 3): walls, towers, buildings, fields and streets over water, approaches through towns or turning sharply, sharp
  turns on network roads; counts plus the worst places (world ft). `TOP=n` lists more, `ROAD=i` prints a road's points.
- `cargo run --release -p worldgen --example placecheck` — settlement placement stats for seed 1 (distance of ports
  to the real shoreline, roads at settlements, capital's road ends vs gates, fishing villages and their piers).
- `cargo run --release -p worldgen --example interior -- <seed> <settlement> <building-index|function>...` —
  building interiors as text plans per level (rooms by letter, `#` stairs, `*` furniture), with timings.
- `cargo run --release -p worldgen --example under -- <seed> [dungeon|crypt|cave|mine|lava_tube...]` —
  underground sites as text plans per level (rooms by letter, `E` way in, `v`/`^` ways down/up, `~` lava).
- `cargo run --release -p worldgen --example probe -- <seed> <fx> <fy> <dx> <dy> [level] [steps]` — height
  profile across a line (terrain vs T0, road mask, water) for diagnosing cuts, banks and seams.

## Rules
- Every generated artifact is a pure function of (world file, job key); dependencies (`pipeline::deps`)
  only point at coarser levels or parent features. Never make output depend on request order.
- Tile samples are a pure function of their global lattice index (see `lod/terrain_refine.rs`); compute
  positions as `global_index * spacing`, never `origin + local * spacing`.
- Bump `world::GEN_VERSION` when output changes for an unchanged world file (and `app/src/world/world.ts`). Edits are
  kept per version (`editsKey`); the site keeps each published version's build, so bump freely, but each one
  published adds ~3.5 MB to the site (1 GB limit). IndexedDB schema changes only add stores (old builds share them).
- No `f64::powi`/`powf` or std transcendentals in generators (platform-dependent rounding); use `libm` or `x * x`.
  Exactly-rounded ops (sqrt, floor, ceil, round, abs) use `core::{sqrt, floor, ceil, round, fabs}` (hardware;
  libm's software versions are several times slower in WASM). Lookup maps use `core::hash::FastMap`/`FastSet`.
- Tests: only vital requirements (determinism, seams/consistency, terrain invariants, battlemap
  guarantee, perf). No per-module unit suites or smoke tests.
- Perf floor: the reference laptop (Intel Iris Xe) must hold ≥ 30 fps 1% lows in the `?bench=1`, `?bench=play`,
  `?bench=sewer`, `?bench=dungeon` and `?bench=edit` runs.
- Sprites: drawn procedurally (`DRAW` in `render/atlas.ts`; furniture and underground props by `InteriorLayer.drawItem`,
  cached in `render/itemAtlas.ts`), in the
  hand-drawn cartoon style the user approves batch by batch (`?gallery=1` shows them all). Generated images
  (`codex exec`) were tried and dropped.

## Git and publishing
- The repo is public: `master` tracks `origin/main`, so a push publishes the source. Commit freely; push only when
  the user asks. Pushing the source doesn't update the site: that's `npm run publish`.
- Never push `private-history` (the pre-public history: it names the user's hosts and personal email) or branches
  built on it (`underdark-wip`: bring its work over by cherry-pick or diff). Commits use the GitHub noreply address
  (this repo's `user.email`).
- Never commit private hosts, emails, keys or local paths: the reverse proxy's `allowedHosts` line in
  `app/vite.config.ts` stays uncommitted, and `docs/authoring-handoff.md` stays git-ignored.
- No license (the user's choice: all rights reserved, source published to be read); don't add one unasked. Code
  that follows someone else's gets its notice in `THIRD_PARTY_NOTICES.md`.
- After visual changes the user wants shown, re-run `scripts/readme-media.mjs` so the README matches the site.
