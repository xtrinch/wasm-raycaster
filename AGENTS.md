# Working in this repository

## Architecture
- Rust in `src/` owns the CPU raycaster, collision queries and WASM bindings.
- `packages/app/src/vector-game/game/` owns the TypeScript game state, input,
  texture loading and persistent WASM buffers. React renders the canvas and HUD.
- `GridMap.wallGrid` is a row-major packed `u64` map. Preserve its wall, door,
  window and floor/ceiling bit layout; see the comments in `gridMap.ts`.
- World geometry belongs in that grid. Stair cells use bit 10, direction bits
  16-17 and an upper-half flag at bit 18; dimensions are shared constants. Generate surface
  buffers from the grid; do not author independent stair/roof geometry lists.
  Movement queries the grid directly. Camera builds stair boxes once and uploads
  them to WASM; do not keep a separate JavaScript copy on GridMap.
- Floors, ceilings and roofs share `src/floors.rs`: a height, visible side,
  grid coverage flags and material. Do not create roof boxes or a roof cache.
- Declare stair cells directly in the `wallGrid` layout using
  `stairCell("north" | "east" | "south" | "west", "lower" | "upper")`.
  Direction means uphill. For a two-cell flight, put the upper cell immediately
  uphill from the lower cell. Do not reintroduce a runtime placement pass.
- World walls are one unit high. Player `position.z` is an offset in renderer
  units: one world unit equals the canvas height. Roofs meet wall tops at z=1.
- Generated bindings live in `packages/app/wasm/` (gitignored). Rebuild them
  whenever a Rust export changes; do not hand-edit generated files.

## Commands and checks
- Install frontend dependencies with `pnpm install`.
- `pnpm run build:wasm` builds the threaded WASM module; requires wasm-pack,
  nightly Rust, rust-src and the wasm32-unknown-unknown target.
- `pnpm run build:app` type-checks and builds the frontend.
- `pnpm run dev` serves Vite with cross-origin isolation for WASM threads.
- `cargo fmt --check` checks Rust formatting. Use targeted tests for geometry
  and movement changes, then check the production build where tools permit.
- Keep new Rust test bodies in `src/tests/`; implementation files only declare their
  test modules with `#[cfg(test)]` and `#[path = "tests/..."]`.
- Keep changes limited to the requested feature or fix; avoid optional refactoring
  and leave existing tests and unrelated code in place.
- `pnpm --filter app test` checks geometry and Player movement (ground WASM
  collision is stubbed). `cargo test` checks Rust projection and depth behavior.
- `cargo test --release render_views -- --ignored --nocapture` runs an optional
  native render/timing harness with synthetic textures and no billboards. Set
  `RAYCASTER_CAPTURE` to an existing directory to save PPM views for inspection.
- Verify rendering and navigation in a browser when possible. Report toolchain
  limitations and distinguish measured FPS from performance expectations.

## Rendering and gameplay changes
- Keep pixel loops in Rust and retain Rayon parallelism. Allocate large buffers
  once in `Camera`, not each frame. Upload static geometry once at initialization.
- Clip geometry against the near plane and screen before rasterization. Depth
  must resolve stairs, roofs, walls and sprites together, including roof views.
- Reuse textures for geometric features. A billboard sprite is not a walkable
  staircase. Share collision/support geometry with rendering.
- Substep movement after slow frames to avoid tunnelling. Keep ground interiors,
  doors and windows usable when adding elevated movement.
- Keep changes scoped; do not commit build output, dependencies or secrets.

- G/H ascend and descend; release both to hover until landing. Keep the flight
  controls when using this saved stairs prototype.
