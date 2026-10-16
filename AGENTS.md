# Working in this repository

## Architecture
- Rust in `src/` owns the CPU raycaster, collision queries and WASM bindings.
- Keep rendering data structs and enums in `src/helpers.rs`, alongside the existing
  types; keep rendering behavior in `src/lib.rs` unless a separate move is requested.
- Keep movement physics and collision decisions in Rust: wall blocking, roof
  support, ceiling limits, landing and gravity. TypeScript handles input, hover
  state and applying Rust's returned position; do not duplicate collision rules there.
- Calculate requested movement distances from speed and frame time in TypeScript;
  pass distances into Rust, as with `walk`, rather than passing frame time.
- `packages/app/src/vector-game/game/` owns the TypeScript game state, input,
  texture loading and persistent WASM buffers. React renders the canvas and HUD.
- `GridMap.wallGrid` is a row-major packed `u64` map. Preserve its wall, door,
  window and floor/ceiling bit layout; see the comments in `gridMap.ts`.
- World geometry belongs in the grid; do not author independent geometry lists.
- The previous stairs/roof prototype is saved on `stairs-roof-prototype`.
  Follow `docs/stairs/NEXT-STEPS.md` for the incremental replacement. This branch
  implements flight and fixed-height roofs; stairs remain on the prototype branch.
- G/H ascend and descend; releasing both hovers until landing. Roof coverage is
  shared by Rust rendering and movement queries, including thin-wall tops.
- World walls are one unit high. Player `position.z` is an offset in renderer
  units: one world unit equals the canvas height. Roofs meet wall tops at z=1.
- Derive horizontal surfaces from `STORY_COUNT` (currently 1): ground at 0,
  floor/ceiling boundaries between stories, and the overhead ceiling/roof at N.
  Do not maintain a separate list of plane configurations.
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
- `cargo test` runs the existing Rust tests. Add targeted checks as each new
  feature is implemented.
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
