# CPU optimization, crate split and WebGPU roadmap

These notes come from reading the code and have not been profiled yet. Add per-pass timing
before acting on the order below.

## 1. Remaining CPU optimizations

Listed roughly from the largest expected win to the smallest.

1. **Walls are drawn through the sprite path.** Every wall, door and window column becomes
   a one-pixel-wide `SpritePart`. In `draw_sprites_wasm`, every screen row loops over *all*
   parts and rejects most of them with a y-range check. That costs about
   rows × (width + windows) checks per frame, plus a sort over all of them. There are two
   fixes:
   - Draw opaque walls in their own column pass and keep only windows in the sorted sprite
     path. This is the better fix; see
     [Banding versus a wall pass](#banding-versus-a-wall-pass).
   - Bucket the parts into row bands, as `draw_meshes` already does for triangles.
2. **Mesh triangle setup has no LOD.** The character models are tens of thousands of
   triangles. Every visible model is transformed, clipped, sorted and banded every frame,
   even when it covers 20 pixels. The main lever is distance-based LOD: choose a
   pre-decimated mesh for each model by its screen size. Culling a whole model by its
   bounding box before the per-triangle loop is a cheaper step that also helps.
3. **Ray–wall intersection is more general than it needs to be.** `raycast_column` uses
   `geo`'s `LineInterval::relate` plus `Euclidean.distance` for every wall slot. Walls are
   always axis-aligned, so a slab test needs only a couple of divides and compares. This is
   the innermost loop of the wall pass.
4. **Allocations and hashing per column and per frame:**
   - Each column allocates a `occupied_cells` Vec and then copies it with `.to_vec()`.
   - The window sprites are cloned.
   - The results go into a SipHash `HashSet`. `rustc-hash` is already a dependency but is
     unused.
   - The `HashMap` texture lookups at the start of `render` also use SipHash.
5. **Overdraw and copies:**
   - The sky is copied across the full screen and then mostly overwritten by the floor and
     ceiling.
   - Presenting a frame makes two full-frame copies on the main thread. The shared buffer
     is copied into an `ArrayBuffer`, then `putImageData` copies it again
     (`Camera.drawCanvas`).
   - Uploading the frame to a WebGL or WebGPU texture would remove one copy.
   - Moving `render` into a worker with `OffscreenCanvas` would also stop it from blocking
     input handling.
6. **SIMD.** `#![feature(portable_simd)]` is enabled, but no code uses it. The
   floor/ceiling texel × light loop and the sprite blend are the obvious candidates.

Skip optimizations that only help the finite map, such as whole-map BVHs or precomputed
visibility, because the map is meant to become infinite.

### Banding versus a wall pass

Row banding removes only the y-range rejections. It does not reduce the per-pixel work, and
a 1-pixel-wide wall part does real work on every row it covers anyway.

The rejection is a couple of integer compares per part per row. As an example, take
1280 × 720 with about 1,400 parts:

- That is roughly 1 M cheap checks per frame.
- Single-threaded, that is on the order of 1–2 ms.
- Spread over the Rayon threads, it drops to a few tenths of a millisecond of wall time.

The saving is largest when walls are far away and short, because most rows then reject most
parts. It is smallest up close, when walls fill the screen and few rows are skipped. Expect
a sub-millisecond gain, unless measurements show the sprite pass dominated by rejections.

A dedicated column pass for opaque walls should do better, because it also cuts the
per-pixel cost:

- Each wall column becomes one vertical span, with no per-row search.
- Opaque walls need no alpha test or blend.
- Storing wall textures transposed (column-major) makes a vertical span read contiguous
  memory. Drawing row-major instead jumps between columns and textures on every pixel.
- The sprite sort only has to cover real sprites and windows, not one part per column.

Walls go through the sprite path because windows are translucent. Glass and anything seen
through it have to be drawn far to near together with the sprites. A column pass keeps that
working if it takes only the opaque wall parts and writes their depth:

- Rays already pass through windows (`stop_at_window` is false), so the wall behind a window
  is an ordinary opaque hit. Only the window part itself is translucent.
- The column pass draws opaque wall and door parts. It tests each pixel against the depth
  buffer and writes to it, just as meshes already do. Above wall tops, a column can have
  several opaque hits; the depth test resolves them, so they can be drawn in any order.
- The sprite pass then sorts and draws only the sprites and window parts, far to near, and
  keeps its per-pixel depth test. A sprite between two walls is clipped correctly by the
  nearer wall's depth, not by the drawing order, and glass blends over whatever is already
  drawn behind it.

The order becomes: floor/ceiling → meshes → opaque wall columns → sprites and windows. The
new cost is one depth write per wall pixel. Any wall or door texture that has transparent
texels has to stay in the sorted path.

## 2. Reusable crates for other projects

Ranked by how useful each would be to other people, weighed against the work needed to
extract it.

1. **Software triangle rasterizer (`draw_meshes`).** This is the easiest one to publish and
   the most self-contained, at about 250 lines.
   - Features: perspective-correct interpolation, near-plane clipping, either winding order,
     and back-face rejection from vertex normals.
   - It runs in parallel row bands with Rayon, which works under wasm threads.
   - It tests against and writes to a caller-supplied depth buffer, so it composes with
     another renderer's output. Here, that is the raycaster's floors and roofs.
   - It is not unique. `euc` also renders into caller-owned color and depth buffers and
     supports shaders. `rusterix` is multi-threaded and tile-based. `softrender` uses Rayon,
     and `retrofire` targets wasm. What this one adds is small: a minimal API and
     wasm-thread-friendly row bands. Publishing it is easy, but it would be one more option
     in a crowded space, not a gap being filled.
   - To make it general: take a camera matrix or projection callback instead of the
     raycaster's `Position`, and let the caller choose the shading (currently a flat color
     with a fixed sun).
2. **Thin-wall grid raycaster** (`raycast_column`, floor/ceiling/roof casting and sprite
   projection). This is the most distinctive code, and the most work to extract.
   - It has sub-cell thin walls, doors, translucent windows, pitch, a vertical camera
     position, and roofs seen from above.
   - This looks like a real gap. A search in November 2026 found one comparable Rust crate:
     `raycaster`, made for Eldiron. It has full-cell walls only, lists doors as unimplemented,
     and has had no pushes since March 2023. `embedded-3dgfx` targets microcontrollers.
   - Non-Rust projects come closer, but as games or tutorials rather than libraries:
     - `TomentRaycaster` (C/SDL2, GPL-2.0) has nearly the whole feature set: offset thin
       walls, sliding doors, see-through doors and thin walls, look up/down, flight,
       multiple floors and multithreaded rendering. It ships as an engine plus a full game,
       and its README does not mention seeing wall tops or roofs from above.
     - `lodev-enhance` (C++ tutorial) has doors, thin walls with transparent pixels, blended
       glass windows and a 2D depth buffer. It has no flight, multiple floors or threads.
     - `raycast.js` has elevation and free look; `BeshRay` lists thin walls as a TODO.
   - So the gap is narrower than "nothing like it". What remains open is a
     permissively licensed Rust library that does this, with roofs seen from above, a
     per-pixel depth buffer that other renderers can draw into, and wasm-thread support.
     `TomentRaycaster` is ahead on multiple floors, which this project does not support.
   - Before publishing, it needs to be decoupled from this game:
     - Replace the packed `u64` layout with a trait the caller implements, for example
       "walls in this cell" and "surfaces in this cell".
     - Replace the `TextureType` enum and texture `HashMap`s with caller-supplied textures.
     - Output to plain `&mut [u8]` RGBA and `&mut [f32]` depth slices.
   - The raw-pointer wasm entry point would stay in this repo as a thin wrapper.
3. **Grid character controller** (`walk`, `walk_step`, `move_vertical` and the
   collision-box queries). It provides substepped walking, wall blocking, box collision,
   gravity, landing and support height. It is useful, but tied to this game's rules, so it
   fits best as an optional module of crate 2, not as its own crate.

Not worth publishing:

- `line_intersection.rs` is a copy of the existing `line_intersection` crate.
- The `Wasm*Array` buffer wrappers are thin glue around `wasm-bindgen`.
- The sky panorama is a strided row copy.
- Model baking already uses the published `posebake` crate.

Extracting any of these crates first requires a Cargo workspace inside this repo, with the
renderer, physics and wasm bindings in separate crates. That split also prepares the
WebGPU work: a GPU renderer becomes a sibling crate instead of a rewrite of `lib.rs`.

## 3. Moving work to WebGPU

Move work over incrementally, starting with a hybrid renderer.

1. **Present and composite on the GPU.** Upload the CPU color and depth buffers as textures
   and draw them with a fullscreen quad. This removes `putImageData`, and the weapon overlay
   can be drawn in the same place. It is low risk and prepares step 2.
2. **Draw meshes on the GPU first.** Triangles are what the GPU is built for, and the CPU
   rasterizer is the part that scales worst with added content.
   - Write the CPU depth into the GPU depth buffer.
   - Draw the meshes with a projection that matches the existing perpendicular-distance
     depth, so they still sort correctly against walls and roofs.
   - Sprites drawn after the meshes must either move to the GPU too or read the depth back.
     Sprites are trivial to draw as quads, so they can move in the same step.
3. **Then move the floor/ceiling/roof pass** into a per-pixel shader. Its output depends
   only on the pixel, the camera and the grid, so it ports almost directly. The grid becomes
   a storage buffer; for the infinite map, upload it in chunks streamed around the player.
4. **Port the wall DDA last.** Casting one ray per column works fine as a compute shader,
   but it is the most intricate code to port. Once the other passes have moved, the CPU
   cost it leaves behind is small.

**Stays on the CPU, in Rust:**

- movement and collision
- gathering the visible cells and models
- level compilation

`wgpu` keeps this work in Rust and is allowed under the no-third-party-engine rule. On
wasm, `wgpu` objects are not `Send`, so GPU work stays on one thread (the main thread or a
worker), separate from the Rayon pool.

## Suggested order

1. Add per-pass timing, so the order of the work is based on measurements.
2. Make the cheap CPU fixes: the wall column pass, the ray–wall test, and the hashing and
   allocation changes.
3. Split the repo into a workspace, then extract the rasterizer crate.
4. Move presentation to the GPU, then the meshes.

Step 1 costs the least and shows whether the GPU is needed at all.
