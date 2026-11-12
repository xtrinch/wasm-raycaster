# Optimization, engine crate and WebGPU roadmap

This document lists proposed work: CPU optimizations ranked by the measured per-pass
timings, the plan for extracting the raycaster into a reusable crate, and the WebGPU plan.
How each pass works, and what is already culled, is in [architecture.md](architecture.md).

## 1. CPU optimizations

### Measured starting point

From the cost model in [architecture.md](architecture.md#10-cost-model): `pnpm run measure`,
headless Edge, 1254 × 627, 8 threads, profiling build, 60-update averages in milliseconds.

| View | Sky | Floors | Walls | Meshes | Sprites | Present |
|---|---|---|---|---|---|---|
| North, spawn view, no models | 0.9 | 5.2 | 0.5 | 0.0 | 2.3 | 1.0 |
| Table and chair in view | 0.7 | 4.4–4.6 | 0.8 | 1.3 | 2.0 | 1.0 |
| One woman in view, close | 0.7 | 4.4 | 0.5 | 3.4 | 1.7 | 0.8 |

These views are all indoors at spawn height. Outdoor views and views from above the roofs
are not measured yet. The road texture and the roof views stress the floor pass differently,
so measure them before ranking floor-pass work by texture cost.

The items below are ordered by the pass they target, largest measured pass first. Every
saving given is an **estimate** from reading the code, not a measurement.

### 1.1 Floors, ceilings and roofs (4.4–5.2 ms)

1. **Skip floor and ceiling pixels hidden behind walls.** The floor pass runs before the
   wall raycast, so it shades every floor and ceiling pixel on the screen, including the
   band between the horizon and each wall's bottom and top edges. The sprite pass then
   paints the wall over that band. In the spawn view the wall covers roughly half the
   screen's height, so a large share of the floor pass is likely spent on pixels that are
   overwritten.
   - Fix: run the wall raycast first, then skip a floor or ceiling pixel when its distance
     is at least that column's `column_depth`. Both use the same perpendicular-depth scale
     (architecture §7). The test is one float compare per pixel and saves the grid read,
     texture fetch and depth write.
   - Only skip behind hits that are fully opaque. Windows already don't set
     `column_depth`, and above wall tops it is ∞, so both cases keep drawing. A door or wall
     texture with transparent texels must not set the skip distance either, or stale pixels
     from the previous frame show through.
   - `column_depth` is the DDA cell-boundary distance, not the exact slot intersection
     (architecture §11), so the cut-off can be off by part of a cell next to thin walls.
     Use the exact hit distance for this test, or skip only beyond a margin.
   - Estimate: up to the hidden fraction of the floor pass, so possibly a few milliseconds
     in wall-heavy views and nothing in open ones. To check, count skipped pixels per view
     before and after.
2. **Draw the sky inside the floor pass.** The sky pass copies the whole panorama window
   (0.7–0.9 ms), and the floor pass then overwrites most of it. The floor pass already
   knows which pixels get no surface (the `None` texture case and the rows at the horizon).
   It can copy the sky texel there instead. With item 1, pixels skipped behind walls need
   no sky either. Estimate: most of the sky pass.
3. **Resolve the grid per cell span, not per pixel.** Every pixel reads its grid cell,
   tests three flag bits and evaluates the texture `match`, although neighbouring pixels on
   a row usually share a cell. Walking the row cell by cell (the same DDA idea as the wall
   rays) resolves the texture once per span and leaves only the texel fetch per pixel.
   Estimate: small to moderate; measure after items 1 and 2.
4. **Mipmaps for distant surfaces.** There is no mipmapping, so far rows sample the large
   road texture (874 × 874 RGBA, about 3 MB) with big strides, which misses the cache and
   aliases. Half-size levels chosen per row by distance would help both speed and quality.
   This only matters outdoors and from above the roofs; measure those views first.

### 1.2 Meshes (up to 3.4 ms per visible character)

1. **Cull whole models against the view before triangle setup.** Models are found per grid
   cell, so a model whose cell a ray reached is fully set up even when almost all of it is
   off-screen. The 1.3 ms mesh pass in the table-and-chair view (under 160 triangles) is
   probably a character just outside the view; this is unverified. Projecting the model's
   footprint box and skipping it when it is entirely outside one side of the view, or
   above or below the screen at the current pitch, is cheap and removes that case.
2. **Level of detail for characters.** A visible character costs all of its 21.7k–32.2k
   triangles at any distance. Pre-decimated meshes chosen by screen size would make distant
   characters cheap. posebake could produce the levels at bake time.

### 1.3 Walls, doors, windows and sprites (1.7–2.3 ms)

Every wall, door and window column becomes a one-pixel-wide `SpritePart`, and the fill loop
checks every part on every row. Walls go through this path because windows are
translucent: glass and whatever is seen through it must be drawn far to near together with
the sprites.

1. **Draw opaque walls and doors in their own column pass.** Each opaque hit becomes one
   vertical span: no per-row search, no alpha test or blend, and a depth test and write per
   pixel, as meshes already do. Only sprites and window parts stay in the sorted pass.
   - Rays already pass through windows, so the wall behind a window is an ordinary opaque
     hit, and only the glass itself is translucent.
   - Above wall tops a column can have several opaque hits. The depth test resolves them,
     so they can be drawn in any order.
   - A sprite between two walls is then hidden by the nearer wall's depth, not by drawing
     order, and glass blends over whatever is already drawn behind it.
   - The order becomes: floors, ceilings and roofs → meshes → opaque wall columns →
     sprites and windows. The new cost is one depth write per wall pixel. A wall or door
     texture with transparent texels has to stay in the sorted path.
   - Storing wall textures column-major would make a vertical span read contiguous memory.
2. **Row banding instead**, as `draw_meshes` does for triangles, is the cheaper change but
   only removes the per-row rejections, which are a couple of integer compares per part per
   row. Estimate: under a millisecond, and nothing when walls fill the screen. Item 1 also
   cuts the per-pixel cost, so it is the better fix.

### 1.4 Present (0.8–1.0 ms)

`Camera.drawCanvas` makes two full-frame copies on the main thread: the shared WASM buffer
into an `ArrayBuffer`, then `putImageData`. Uploading the frame as a WebGL or WebGPU texture
removes one copy; see [section 3](#3-webgpu). Rendering in a worker with `OffscreenCanvas`
would also stop the frame from blocking input handling.

### 1.5 Not worth doing now

The wall raycast measures 0.5–0.8 ms, so these are small however they turn out:

- **An axis-aligned slab test instead of `geo` intersections** in `raycast_column`. Walls
  are always axis-aligned. It is still worth doing for the engine crate, where it removes
  the `geo` dependency and the copied `line_intersection.rs`.
- **Per-column allocations and hashing.** `raycast_column` allocates `occupied_cells` and
  copies it with `.to_vec()`, the hits are cloned, and the cells go into a SipHash
  `HashSet`. This is per-frame bookkeeping of a few hundred entries.
- **SIMD.** There is no SIMD code; the unused `portable_simd` feature was removed in the
  crate split, which also keeps the engine on stable Rust. Revisit for the floor texel loop once items 1.1.1–1.1.3 have changed its shape.

Skip optimizations that only help the finite map, such as whole-map BVHs or precomputed
visibility, because the map is meant to become infinite.

## 2. The engine crate

The goal is to publish the raycaster as `raycast-engine` (free on crates.io as of
2026-11-03), licensed MIT OR Apache-2.0, while the game keeps working.

### Why it is worth publishing

- Searches in November 2026 found no permissively licensed Rust library with this feature
  set: thin walls inside cells, doors, blended see-through windows, pitch, flight, roofs
  seen from above, a per-pixel depth buffer that 3D meshes composite into, and Rayon
  threads in wasm.
- The closest Rust crate is `raycaster`, made for Eldiron. It has whole-cell walls only,
  lists doors as unimplemented, and has had no pushes since March 2023.
- The closest project overall is `TomentRaycaster` (C/SDL2, GPL-2.0). It has thin walls,
  see-through doors, flight, multiple floors and threads, but ships as an engine bundled
  with a full game, and its README does not mention roofs seen from above. It is ahead of
  this project on multiple floors.
- So the README should present a reusable Rust and wasm library, not a first of its kind.
- The triangle rasterizer on its own is not novel (`euc`, `rusterix`, `softrender` and
  `retrofire` exist), so it is not published separately.

### Decisions

- **One crate with everything**, including `draw_meshes`: `World` (the editable level), one
  `render`, and collision (`walk`, `move_vertical`). No hooks, feature flags or optional
  modules. How meshes reach the GPU is decided when that work starts.
- **The game** keeps the wasm bindings and the TypeScript app: input, texture loading, OBJ
  parsing and the HUD.
- **Levels are built by `World`**, which is done (PR #7). TypeScript describes the level as
  placements in `level.ts`. A data-file loader (JSON or RON) can later sit on the same
  `World` for engine users who do not write TypeScript.
- **The packed grid stays a first-class, documented part of the engine.** A GPU renderer
  needs packed data to upload, in chunks for the infinite map, so the engine is not built
  around a per-cell callback trait.
- **Repository:** extract the crate as a Cargo workspace in this repo first, with
  `crates/raycast-engine` and the game staying the root wasm package. Once the API settles,
  move it to a new engine-first repository with a purpose-built demo, keeping history with
  `git subtree split --prefix crates/raycast-engine`. No git submodules. After the move the
  game depends on a git revision or the crates.io release, with a gitignored
  `.cargo/config.toml` `[patch]` pointing at a local checkout for development.
- **Assets:** the engine repository and its demo ship only CC0 or own art. Some of the
  game's textures are Freepik uploads, and the terms for the rest are unknown.

### Steps

1. **Snapshot tests** (done). Render fixed scenes natively through `render`, built with `World`
   and procedural textures, and compare them with reference PNGs: walls, a door and a
   window; a sprite behind glass; a mesh; a roof seen from above while pitched down.
2. **Workspace split** (done).
   - `crates/raycast-engine` holds the world, the render passes and collision, with no
     `wasm-bindgen`, `js-sys` or `web-sys`. It must build on stable:
     `cargo +stable check -p raycast-engine`.
   - The root crate keeps only the wasm bindings (`WasmWorld`, the buffer and texture
     wrappers) and the raw-pointer `render` entry point.
   - Replace `js_sys::Math::atan2` with `f64::atan2`, the only browser call in the render
     passes.
   - Per-pass timing used `web_sys` `performance.now()`. `render` now takes a clock, and
     the wasm wrapper passes `performance.now()`.
   - Drop the unused `#![feature]`s from the engine.
   - `TextureType` is defined in both crates for now, with a test that the values match.
3. **API cleanup.** Textures supplied by the caller instead of `TextureType` and the
   texture `HashMap`s; a `Camera` type instead of `Position` and `plane_y_initial`; output
   to a frame of colour and depth slices; scratch buffers allocated once in a renderer
   object.
4. **Publish.** A README with screenshots, a native example that writes a PNG, a minimal
   wasm example, and documented limits: one story, at most three segments per cell, 0.1
   precision. Then release 0.1.
5. **Move** to the new repository with the demo, as described above.

## 3. WebGPU

Move work over incrementally, starting with a hybrid renderer. `wgpu` keeps the work in
Rust and is allowed under the no-third-party-engine rule.

1. **Present on the GPU.** Upload the CPU colour buffer as a texture and draw it with a
   fullscreen quad. This replaces `putImageData` (item 1.4), and the weapon overlay can be
   drawn in the same place (architecture §11 notes it is currently overwritten).
2. **Meshes on the GPU.** Triangles are what the GPU is built for, and the CPU rasterizer
   scales worst with added content.
   - Write the CPU depth into the GPU depth buffer, and project meshes so their depth
     matches the perpendicular-distance scale, so they still sort against walls and roofs.
   - The sprite pass runs after the meshes, so it either moves to the GPU too or reads the
     depth back. Sprites are simple quads, so they can move in the same step.
3. **Floors, ceilings and roofs** as a per-pixel shader. The output depends only on the
   pixel, the camera and the grid, so it ports almost directly. The packed grid becomes a
   storage buffer, uploaded in chunks around the player for the infinite map.
4. **The wall DDA last.** One ray per column works as a compute shader, but it is the most
   intricate code to port, and it is already the cheapest pass.

**Stays on the CPU, in Rust:** `World` and its edits, movement and collision, and finding
the visible cells and models.

On wasm, `wgpu` objects are not `Send`, so GPU work stays on one thread (the main thread or
a worker), separate from the Rayon pool.

## Suggested order

1. Measure the views that are missing: outdoors, and from above the roofs.
2. Floor-pass items 1.1.1 and 1.1.2, then model culling (1.2.1). They target the two
   largest passes and do not change any API.
3. Engine steps 1–2 (snapshot tests, workspace split). The snapshot tests also guard the
   floor-pass changes, so they can come first if those changes feel risky.
4. The opaque wall column pass (1.3.1).
5. GPU present, then GPU meshes, if the CPU numbers still fall short of the target frame
   time.
