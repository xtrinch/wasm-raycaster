# Stairs and roof access

The exterior staircase connects ground level to the top of the existing building.
It consists of twelve solid steps encoded in `GridMap.wallGrid`, the source of
truth for world geometry. Rendering derives surface boxes from the grid, and
movement queries those same cells directly. The roof is a walkable surface at
the existing wall tops. A small shed
beside the stair foot provides a reference when observing ascent and descent.

This guide describes the current implementation, its constraints, and how to
change it without breaking interior rendering or movement.

## Contents

- [Try it](#try-it)
- [Source map](#source-map)
- [Coordinates and dimensions](#coordinates-and-dimensions)
- [Geometry and placement](#geometry-and-placement)
- [Movement and support](#movement-and-support)
- [Rendering and occlusion](#rendering-and-occlusion)
- [Materials](#materials)
- [Performance](#performance)
- [Build and tests](#build-and-tests)
- [Changing the implementation](#changing-the-implementation)
- [Troubleshooting](#troubleshooting)
- [Current limitations](#current-limitations)

## Try it

From the repository root, with Node, pnpm, Rust and wasm-pack installed:

```sh
pnpm install
pnpm run build:wasm
pnpm run dev
```

Open the URL printed by Vite. The configured application base is
`/wasm-raycaster/`. The WASM build uses nightly Rust, `rust-src`, the
`wasm32-unknown-unknown` target and the `parallel` feature. Vite supplies the
cross-origin isolation headers required by the threaded WASM runtime.

The player starts inside the main building. Leave through an existing doorway
and go around to the **outside of the north wall**.

Approach `(4.45, 0)`, face south (increasing `y`),
and walk forward. Height follows the steps automatically. At the top, continue
onto the roof. To descend, face north (decreasing `y`) and walk down the same
staircase. No jump key is required. The layout explicitly declares the lower stair at `(4, 0)` and the upper stair
at `(4, 1)`, both ascending south. Placement is fixed by those map entries.

| Key | Action |
| --- | --- |
| Up / Down arrow | Walk forward / backward |
| Left / Right arrow | Turn |
| A / S | Look up / down |
| D / F | Raise / lower the existing vertical offset, limited by the walking surface |

Pitch returns toward the center automatically. Walking over an unguarded roof
edge lowers the player toward ground level; there are no railings or fall damage.

## Source map

Paths below are relative to the repository root.

| File | Responsibility |
| --- | --- |
| [`elevation.ts`](../../packages/app/src/vector-game/game/elevation.ts) | Stair-cell encoding/decoding, derived step boxes and grid support queries |
| [`gridMap.ts`](../../packages/app/src/vector-game/game/gridMap.ts) | Packed map layout, directional stair entries and material loading |
| [`player.ts`](../../packages/app/src/vector-game/game/player.ts) | Substepped walking, stair entry, support snapping and vertical offsets |
| [`camera.ts`](../../packages/app/src/vector-game/game/camera.ts) | Persistent buffers, geometry upload and WASM calls |
| [`lib.rs`](../../src/lib.rs) | Frame orchestration, existing raycaster, wall/sprite occlusion and background |
| [`floors.rs`](../../src/floors.rs) | Shared floor/ceiling/roof rendering at any height, directly from grid coverage |
| [`stairs.rs`](../../src/stairs.rs) | Stair box projection, clipping, scan conversion and texture sampling |
| [`helpers.rs`](../../src/helpers.rs) | WASM containers, texture IDs and sprite depth field |
| [`elevation.test.js`](../../packages/app/elevation.test.js) | Geometry and Player regression checks |
| [`tests/render.rs`](../../src/tests/render.rs) | Native interior/background regressions and optional render harness |

## Coordinates and dimensions

`x` increases east and `y` increases south. One map cell is one horizontal world
unit. Walls are one world unit tall; the roof is at world height `1`, and the
ground is at `0`.

| Setting | Current value |
| --- | --- |
| `STEP_COUNT` | `12` |
| `STEP_HEIGHT` | `1 / STEP_COUNT`, approximately `0.08333` |
| `STAIR_START_Y` | `0.1` |
| `STAIR_END_Y` | `2.0` |
| Total horizontal run | `1.9` world units |
| Depth of each tread | `1.9 / 12`, approximately `0.15833` |
| `STAIR_WIDTH` | `0.9` world units |
| Stair cells | Lower at `(4, 0)`, upper at `(4, 1)`, ascending south |
| Roof height | `1` world unit, exactly at the wall tops |

`position.z` uses renderer units rather than world units:

```text
feet world height = position.z / camera.height
eye world height  = 0.5 + feet world height
roof standing z   = camera.height
```

The canvas height is a conversion factor, not a change to the building's physical
height. The roof must stay aligned with wall tops. The current Camera sets its
dimensions at construction; this feature does not add live resize handling.

## Geometry and placement

### Grid cells are the source of truth

The staircase is declared directly in the `wallGrid` array, alongside wall and
ground entries. `stairCell()` encodes one cell's value; it never chooses a
coordinate or modifies other cells. There is no runtime stair-placement pass.

The current layout contains:

```ts
// In row 0, column 4:
stairCell("south", "lower")
// In row 1, column 4:
stairCell("south", "upper")
```

Each cell stores only its lower/upper section and direction. Direction always
means **uphill**, so descending means walking the opposite way. Both entries
use the same direction:

| Uphill direction | Upper cell relative to lower cell `(x, y)` |
| --- | --- |
| `"north"` | `(x, y - 1)` |
| `"east"` | `(x + 1, y)` |
| `"south"` | `(x, y + 1)` |
| `"west"` | `(x - 1, y)` |

The lower section begins 0.1 units inside its entry edge. The upper section
continues the same 1.9-unit, twelve-step flight to its exit edge at roof height.
One tread crosses the cell boundary; its pieces share the same height and are
merged for rendering. Upper/lower are sections of one continuous flight, not
independent ground-to-roof staircases.

For other locations or orientations, change the actual array entries. For example,
an eastward pair uses `stairCell("east", "lower")` at `(x, y)` and
`stairCell("east", "upper")` at `(x + 1, y)`. The same decoder handles rendering
and movement in all four directions.

| Bits | Stair-cell meaning |
| --- | --- |
| `10` | Stair-present flag (`0x400`) |
| `12-15` | Wall-segment count, zero for stairs |
| `16-17` | Uphill direction: north=0, east=1, south=2, west=3 |
| `18` | Section: lower=0, upper=1 |

No dimensions are packed into individual cells. `STEP_COUNT`, `STAIR_HEIGHT`,
`STAIR_WIDTH`, `STAIR_START_Y` and `STAIR_END_Y` in `elevation.ts` define the shared
shape. The decoder reads only direction and lower/upper; rendering and movement
use those same constants to calculate the steps.

The encoder preserves the road material in the low flags. Stair cells contain
no wall segments: declare a stair entry in a free cell rather than combining
its payload with an existing wall payload.

The upper bits have this meaning only for stair cells. Existing wall cells keep
their existing segment layout. No wall payload is repurposed while it is in use.
Each occupied cell carries its direction and section, and contributes geometry only within
its own footprint. Removing one cell removes its part of the flight from both
derived geometry and support queries. A copy of the packed grid can reconstruct
the staircase without a separate coordinate or geometry list.

`buildStairBoxes(grid, size)` takes no stair-position argument. It reads
stair cells, clips tread boxes to their owning cells, then rejoins
adjacent pieces of the same tread. A tread crossing a cell boundary therefore
still renders as one box. The normal staircase remains twelve render boxes.

Camera builds the step boxes once from the grid and uploads them to WASM. The
temporary JavaScript boxes are then discarded; Camera keeps only their count
and the WASM buffer. GridMap stores no step boxes. Roofs are drawn directly from
grid cells by the floor renderer. Placement and uphill direction come from the
cell entries; there is no independent stair coordinate list.

### Derived boxes

Each `SurfaceBox` is a tuple of six numbers:

```ts
[x0, y0, x1, y1, bottom, top]
```

The horizontal containment test includes the minimum edges and excludes the
maximum edges: `x0 <= x < x1` and `y0 <= y < y1`. This assigns a point at a tread
boundary to a single step.

For zero-based step index `i`, the current descriptor yields these boxes after
cell pieces are rejoined:

```text
x0     = 4
x1     = 4 + STAIR_WIDTH
y0     = STAIR_START_Y + i       * treadDepth
y1     = STAIR_START_Y + (i + 1) * treadDepth
bottom = 0
top    = (i + 1) / STEP_COUNT
```

These are solid boxes extending to the ground, rather than floating tread slabs
or billboard sprites. The last box reaches height `1`.

### Meeting the building

The main north wall occupies `y=2.0` through `y=2.1` at the stair candidates. The
stairs end at its **outer face, `y=2.0`**. The roof begins over the roof-marked map
cells and provides the next walking surface across the wall top.

Do not extend the staircase to the inner face at `y=2.1`. That places its back
face on the interior wall face, causing equal-depth surfaces to compete and the
stair material to appear inside. The JavaScript tests decode the actual packed
wall coordinates to check this boundary.

### Roof footprint

Roof cells are identified by bit `1` (`2n`) of `wallGrid`, matching the interior
floor/ceiling footprint. The shared floor renderer projects a horizontal plane
at height `1`, then checks the grid at each sampled world position. A roof exists
only where the sampled cell carries that flag. Courtyard gaps stay open.

There are no roof rectangles, vertices, box lists or separate roof uploads.
Movement support also reads the grid flag directly.

### Stairside shed

The shed is also declared directly in the map: row `0`, columns `5` and `6`
contain its packed wall values. Each cell has three wall segments; their shared
internal edge has no wall. The resulting shed measures two cells east-west by
one cell north-south. Moving the staircase does not automatically move the shed;
edit its map entries too when needed.

Its outer faces align with the cell boundaries and roof footprint. Wall thickness
is `0.1`, extending inward: the minimum-side offsets are `0`, and maximum-side
offsets are `0.9`. Insetting all faces while retaining full-length spans produces
protruding corners. The shed uses the normal wall raycaster and roof system.

## Movement and support

`GridMap.supportHeight()` delegates to `gridSupportHeight()`, which looks up the
packed cell under the player. `GridMap.stairHeight()` uses `stairHeightAt()` to
decode its stair cell and identify the tread. Neither scans the render boxes:

1. A point inside a marked stair cell and its encoded footprint gets that step's
   top height. An unmarked cell cannot provide stair support.
2. Roof support is available only when the feet are at least
   `1 - STEP_HEIGHT - 0.001` high. A player on the ground therefore stays under
   the roof instead of snapping upward while walking inside the building. Roof
   support comes directly from the cell's existing bit 1.
3. Otherwise, the support height is ground level, `0`.

`Player.walk()` divides horizontal movement into pieces no longer than `0.05`
world units. `update()` also caps the simulated frame time at `0.1` seconds. These
limits keep ordinary movement from skipping an entire tread after a slow frame.

For each piece, `walkStep()` checks the candidate position. A stair whose top is
more than `STEP_HEIGHT + 0.001` above the current feet blocks that move, so the
player cannot enter a high riser sideways from the ground. Movement onto the
lowest tread, along the stairs, or with feet already at roof height can bypass
the ground-wall query. Other movement uses the existing Rust `walk()` collision
routine.

When the player is supported within `0.002` world units and the next surface
differs by no more than one step plus tolerance, the feet snap to that surface.
This follows treads both upward and downward without floating above them. Larger
drops use the existing downward offset decay of `100` renderer units per second.
That decay is not an acceleration-based gravity simulation.

The D/F controls remain vertical-offset controls. The offset is capped at `300`
renderer units above support, and lowering is clamped to the walking surface.
They do not implement a ballistic jump or a crouching animation.

## Rendering and occlusion

### Frame pipeline

```text
Background panorama
    -> shared floor/ceiling/roof pass + per-pixel depth
    -> wall raycast and sprite discovery
    -> stair boxes, sharing that depth buffer
    -> walls and sprites, tested against surface depth
    -> canvas presentation
```

Camera flattens the stair boxes into a `Float32Array`, uploads them to a
`WasmFloat32Array` once, and passes the pointer and box count to `render()`.
Each box occupies six floats, or 24 bytes.

`stairs::draw()` constructs the visible faces of each box. Top faces are emitted
when the eye is above them. Vertical faces are selected according to the camera's
side of the box. This box renderer handles stairs only.

### One renderer for every floor height

`floors::draw()` handles floors, ceilings and roofs through the same `FloorPlane`
data: world height, visible side (up or down), grid coverage flags and material.
An optional alternate material lets ground cells choose the road texture.

The current scene declares ground at height `0`, a downward-facing ceiling at
height `1`, and an upward-facing floor at height `1` using the roof material.
The latter two are the opposite sides of the same building level. Each uses the
same row-distance projection, grid lookup, texture sampling and depth comparison.

For each row, distance to a horizontal plane is constant. The renderer computes
that distance once, then steps across world coordinates using wide fixed-point
increments. It shades only marked grid cells that are nearer than existing depth.
Rows run in parallel with Rayon. There is no special roof projection or roof mesh.

Where a cell has an exposed wall without a full roof, the same plane pass draws
the wall's top using its packed thickness, length, orientation and offsets.
`FloorPlane.wall_tops` uses the same concrete material as the roof. Vertical
faces retain the wall material. This fills the actual wall footprint
instead of leaving only the edge of a vertical face visible from above. Existing
roof-covered cells keep their roof material. No wall-top boxes or extra drawing
pass are created.

The renderer accepts other plane heights through the same API; tests exercise a
floor at height `2`. Full multiple-story gameplay still needs story-specific map
data, walls and movement support, but it will not need a separate drawing method
for each story's floor.

Faces crossing the near plane are clipped at forward distance `0.02`. Surviving
polygons are projected and rejected when entirely outside a screen side. The
renderer then processes rows in parallel with Rayon, intersects polygon edges
with each scanline and shades only the covered horizontal span.

For a covered pixel, a ray/plane intersection supplies the world position and
depth. Texture coordinates therefore follow the surface in perspective. Horizontal
faces use world `(x, y)`; vertical faces use the horizontal wall coordinate and
`1 - worldZ`, keeping the material upright. Coordinates repeat once per world unit.

### WASM buffers in Camera

`stairBoxesRef` is the WASM-memory copy of the stair boxes derived from
`wallGrid`. Camera flattens each box into six consecutive floats:
`[x0, y0, x1, y1, bottom, top]`. It uploads this array once and passes
`stairBoxesRef.ptr` and the box count to Rust each frame. Rust reads it to project
and draw the surfaces. It contains world-space bounds, not screen coordinates
or pixel depths. The `Ref` suffix refers to the wrapper around WASM-owned memory.

| Camera field | Contents | Updated |
| --- | --- | --- |
| `mapRef` | Packed `wallGrid` cells, including directional stair entries | Uploaded at initialization |
| `stairBoxesRef` | Stair boxes decoded from those cells for the box renderer | Uploaded at initialization |
| `zBufferRef` | Wall depth per screen column | Every frame |
| `surfaceDepthRef` | Nearest floor, ceiling or stair depth per screen pixel | Every frame |

The wall and floor renderers read packed cells directly. The stair box renderer
instead reads these predecoded boxes, avoiding repeated step decoding
each frame. `stairBoxesRef` is a derived rendering cache; edit `wallGrid` to change
the world, then rebuild its derived buffers if the map changes after startup.

### Depth and roof views

`surfaceDepthRef` stores the nearest floor, ceiling or stair depth for each pixel.
The shared floor pass resets it to infinity each frame and draws horizontal planes.
The stair pass keeps those values and replaces them only with nearer steps.
Depth comparisons resolve overlap regardless of plane/box order.
The wall/sprite pass compares its projected depth against this
buffer. This lets a roof hide the lower part of a tree or wall without hiding
the whole screen column.

When `position.z >= height / 2`, the eye has reached wall-top height. Wall
raycasting stops treating the first wall as a full-column visibility barrier and
continues collecting farther geometry. The column wall buffer becomes infinity
for these elevated views. Fully surface-hidden wall stripes are filtered before
the row/sprite drawing loop to limit the extra work.

The depth buffer now includes all horizontal floors and ceilings as well as
stairs. Walls and sprites still use their existing ordering while checking that
depth; this is not a complete replacement of the older wall/sprite renderer.

### Pitched background coverage

Looking up or down can expose rows beyond the prescaled panorama. The background
extends its first or last row instead of skipping those screen rows. Copy bounds
also include the final source row. This ensures uncovered floor edges receive a
new background every frame, rather than retaining and smearing previous textures.

## Materials

| Surface | Texture ID | Runtime asset |
| --- | --- | --- |
| Roofs, including shed roof | `ROOF = 13` | [`roof-concrete.jpg`](../../packages/app/src/assets/roof-concrete.jpg) |
| Stair treads, risers and sides | `STAIRS = 14` | [`stair-concrete.jpg`](../../packages/app/src/assets/stair-concrete.jpg) |

The roof uses lighter weathered concrete and the stairs use darker rough concrete.
Both were generated with imagegen. Their original PNGs and the
[generation prompts](../../packages/app/src/assets/concrete-textures.md) are kept
in the assets directory. Optimized JPEGs total roughly 38 KB and are loaded as
256 Ã— 256 RGBA textures: 256 KiB each in WASM, excluding other copies and metadata.

Interior `FLOOR` and `CEILING` remain separate materials. Generated images are
static assets with random-looking mottling; they are not regenerated per frame
or randomly changed each time the game starts.

Camera waits for 21 texture entries, including directional variants, before
rendering. A new texture ID must be handled by
`GridMap.getSpriteTexture()` and `getSpriteData()` as well as the Rust enum.

## Performance

- Geometry is derived from the packed grid and uploaded once, and large
  color/depth buffers are allocated once by Camera. Changing the grid after
  Camera construction requires rebuilding the step boxes and updating both
  its wall-map and surface uploads; this feature does not add a live map editor.
- Roofs need no projected polygons or geometry upload. They use the same row-based
  grid sampling as floors and ceilings. The staircase stays at twelve boxes.
- Near clipping, screen rejection, face selection and scanline spans limit pixel
  work. Existing Rayon parallelism is retained.
- Projection still allocates small face/polygon vectors each frame. The renderer
  is not allocation-free.
- The additional surface depth buffer costs `4 * width * height` bytes: about
  1.98 MiB at 960 Ã— 540 or 7.91 MiB at 1920 Ã— 1080. It is cleared and accessed
  each frame, so higher resolutions increase both memory traffic and shading work.
- Roof views discover more walls than ground views. Avoid removing hidden-stripe
  filtering or adding many overlapping surfaces without measuring the result.

The native harness is useful for relative renderer measurements, but uses
synthetic textures and no billboard sprites. It does not establish browser FPS.
For browser comparisons, use the same viewport, build profile, scene, camera path
and hardware. Warm up loading before observing the HUD's average/minimum FPS.
Check the interior, stair approach, ascent, roof traversal and descent separately.

## Build and tests

### Build

```sh
pnpm run build
```

The normal WASM script uses the profiling profile. For an optimized local build
with the same output name:

```sh
wasm-pack build --release --target web --out-dir packages/app/wasm --out-name in -- --features parallel
pnpm run build:app
```

Rust export or texture-enum changes require rebuilding the generated bindings
before type-checking the frontend. The bindings in `packages/app/wasm/` are
gitignored; do not hand-edit them. A material-image or TypeScript placement change
does not itself require recompiling Rust.

### Automated checks

```sh
pnpm --filter app test
cargo test
cargo fmt --all --check
```

On Windows, the explicit native target can reuse the same target directory as
the verification performed during development:

```powershell
cargo test --release --target x86_64-pc-windows-msvc
```

JavaScript tests cover packed-grid round trips, cell removal affecting both
rendering and support, bare maps containing no stairs, explicit map entries,
direction/section validation, movement in all four directions, exact shed wall bounds, stair/wall
separation, roof footprint gaps, ascent/descent, high-side entry and roof-edge
falling. They transpile the TypeScript modules and exercise Player with the
ground WASM collision call stubbed as unobstructed movement. They do not validate
the Rust collision implementation or browser input.

Rust tests live in `src/tests/`, separate from the implementation files. The
implementation modules contain only test-module declarations to retain access
to private helpers. Tests cover near clipping, finite surface depth, order-independent nearest
surfaces, alignment with wall tops, upright vertical texture mapping, complete
pitched-background coverage and exterior stairs staying hidden behind interior
walls. The interior regression compares renders with and without stairs at
multiple positions, heights and pitches using opaque fixture textures.

### Optional native captures and timings

```sh
cargo test --release render_views -- --ignored --nocapture
```

To save the five PPM views in PowerShell:

```powershell
New-Item -ItemType Directory -Force target/render-captures | Out-Null
$env:RAYCASTER_CAPTURE = Join-Path (Get-Location) 'target/render-captures'
cargo test --release --target x86_64-pc-windows-msvc render_views -- --ignored --nocapture
```

The directory must already exist. Captures are synthetic views of the interior,
stair approach, climbing, roof, roof edge and thin-wall top at 960 Ã— 540. The native fixture
reconstructs the base map and its own staircase; it does not load the JPEGs or run the TypeScript cell encoder. The native
fixture substitutes matching packed values for the two declared stair entries;
it reads the shed walls from the map literal. Update fixture geometry
when stair dimensions change. Keep captures under gitignored `target/`.

### Manual browser checks

1. Find the stairs using the HUD; climb and descend without D/F.
2. Turn and move backward on treads; check that height follows the support.
3. Walk across the wall top onto the roof and inspect courtyard gaps.
4. Observe the shed from the top and while descending; its corners should meet.
5. Inspect the north wall from inside: no stair rear face should replace it.
6. Hold S and move back and forth at the roof/stair edge: no texture trails.
7. Check that roof/stair concrete differs from the interior floor and ceiling.
8. Walk over a roof edge and confirm descent to ground, then re-enter a doorway.

## Changing the implementation

**Step count or size:** edit the shared constants in `elevation.ts`.
`stairCell()` accepts only direction and lower/upper; dimensions are never
arguments or per-cell data. Both halves always use the same shared dimensions.
Keep the final top at `1`, treads contiguous, and each tread deep enough for the
movement substep. Changing `STEP_COUNT` also changes the permitted step height
and roof-support tolerance. Update the native fixture and rerun both suites.

**Placement and orientation:** edit the actual `wallGrid` entries. Put the upper
cell immediately uphill from its lower cell, using the same direction.
Verify an unobstructed approach and a roof at the upper exit.
The current pair is `(4, 0)` / `(4, 1)`, ascending south to the building's north
wall. Its shed is separately declared at `(5, 0)` / `(6, 0)`.

**Textures:** replace or version the appropriate JPEG and update its Bitmap
dimensions if needed. Keep an opaque, uniformly lit repeating albedo without
perspective or baked directional shadows. Preserve the material ID separation
and the vertical `1 - worldZ` convention.

**Height:** the one-unit wall/roof assumption is shared by generation, support
queries, player thresholds and projection. A taller staircase or second floor
requires coordinated changes; editing only a box top will not provide correct
multi-level movement.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| Stairs appear through an interior wall | Verify their end is at the outer face, not the inner face; check surface/wall depth conventions. |
| Vertical texture is upside down | Vertical UVs must use `1 - worldZ`; horizontal UVs use `(worldX, worldY)`. |
| Texture trails while looking down | Every background row must be replaced, including clamped panorama rows and exact-end copies. |
| Shed corners protrude | Keep outer faces at the footprint bounds and thickness inward; do not combine inset faces with full-cell spans. |
| Ground player snaps to the roof | Preserve the feet-height gate in `supportHeight()`. |
| Player floats down the steps | Preserve support-aware downward snapping within one step. |
| Roof or stairs use an interior texture | Check `ROOF`/`STAIRS` enum values, GridMap material cases and `surface_texture()` routing. |
| Missing texture or initialization failure | Rebuild WASM after enum changes and verify every expected texture finishes loading. |
| Native capture disagrees with the game | Check duplicated fixture dimensions; captures use synthetic materials and omit billboards. |

## Current limitations

This is a single-building, single-roof-height extension to the existing raycaster.
Stairs are authored in map cells with one of four cardinal uphill directions.
The current two-cell section dimensions are fixed by the encoder's lower/upper
profile; there is no automatic routing, random placement or terrain fitting. There
are no slopes, railings, walkable stair undersides, stacked floors, or generalized
3D collision bodies. Roof support follows marked map cells, and roof sides reuse
the existing walls. Gravity and jumping retain the older renderer-offset model.

These constraints keep the change small and let the existing interior remain
usable. Extending it into a general multi-level engine requires a broader support,
collision and depth design rather than additional placement constants.
