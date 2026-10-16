# Smaller steps for stairs and wall heights

The current stairs work, but the changes are difficult to review. Preserve that
version, then build the underlying features in small, separately reviewable steps.

## 0. Preserve the current work

- [x] Save the current implementation on `stairs-roof-prototype`. Required textures
      were already tracked in the base history.
- [ ] Start a separate branch from the version before the stairs changes.
- [ ] Keep each step below in its own change. Avoid unrelated cleanup or refactoring.

## 1. Flight and roof movement

- [x] Add G/H jetpack controls for ascending and descending. Release both to hover;
      landing turns flight off. Preserve the existing D/F controls.
- [x] Render roofs at the tops of the existing fixed-height walls with concrete.
- [x] Fly onto a roof, land and walk on it, block entry through wall sides, and
      fall when walking off an edge.
- [ ] Check views from below, above, and beside roofs, including looking down at
      thin wall tops. Check occlusion and the previously observed edge smearing.
- [ ] Check performance before proceeding. No stairs or wider grid format yet.

## 2. Support variable-height walls

- [ ] Agree on height precision, range, and the smallest necessary grid-format change.
- [ ] Store wall heights in the grid and make existing wall rendering read them.
- [ ] Render wall tops at their actual heights using the horizontal surface renderer.
      A wide, short wall is a platform; no separate raised-platform feature is needed.
- [ ] Extend roof walking support to the different wall-top heights.
- [ ] Ensure short walls reveal taller walls behind them and collision respects height.
- [ ] Use flight controls to inspect a small test layout with different wall heights.
- [ ] Verify existing doors, windows, and ground-level rendering still work.

## 3. Revisit stairs after those foundations work

- [ ] Count the fields needed for explicit geometry before choosing a wider cell size.
      The discussed 512-bit layout is a proposal, not an agreed format.
- [ ] Decide how to author readable map data and pack it at build time, so runtime
      rendering does not generate stair geometry.
- [ ] Represent steps using ordinary walls with heights; their tops are the treads.
- [ ] Add automatic stepping up and down small height differences.
- [ ] Reuse the saved branch's textures, fixes, and relevant tests where useful.
- [ ] Remove the special stair-box buffer and renderer once the shared path replaces them.

## Later cleanup

- [ ] Bring back the flight/movement tests saved on `stairs-roof-prototype`.

- [ ] Add broader floor-rendering tests for plane heights, visibility, depth order,
      gaps and thin-wall tops alongside the later renderer work.
- [ ] Move `draw_ceiling_floor_raycast` and its supporting types from `src/lib.rs`
      into a dedicated floor-rendering module after the behavior changes are reviewed.
      Keep the current name and location while reviewing flight and roof support.

Finish and review each step before starting the next. Keep comments short and
describe packed fields in plain terms.
