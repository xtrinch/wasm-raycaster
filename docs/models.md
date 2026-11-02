# How 3D models are drawn

This document follows a 3D model from its file to the screen. Models are drawn by a CPU
triangle rasterizer in Rust (`draw_meshes` in `src/lib.rs`) and composited with the
raycaster's floors, walls, windows and billboard sprites through one shared depth buffer.
They behave like sprites with a mesh: the grid decides whether they are drawn at all, and
anything nearer covers them.

Pipeline at a glance:

```
OBJ + MTL ──parseObj──▶ Mesh ──compileLevel──▶ meshTriangles + models table
                                                    │ uploaded once (Camera)
                                                    ▼
every frame:  raycast finds occupied cells ─▶ visible models ─▶ draw_meshes
              floor pass ─▶ mesh pass ─▶ sprite pass   (one shared depth buffer)
```

## 1. Files: OBJ and MTL

Models are Wavefront OBJ text files in `packages/app/src/assets/models/`, optionally with an
MTL file holding material colours. `parseObj` and `parseMtl` in
`packages/app/src/vector-game/game/objModel.ts` read only these lines:

```
v  0.04281 1.56658 -0.05468      corner position x, y, z; numbered 1, 2, 3, ... in file order
vn 0.12    0.98    0.15          normal: which way the surface faces at a corner
usemtl c12                       following faces use material c12
f  12//12  13//13  14//14        a face: corner numbers, each with an optional normal number
```

- A face's corners may be written `v`, `v/vt`, `v//vn` or `v/vt/vn`. Texture coordinates are
  ignored, and negative numbers count back from the most recent entry.
- A face with more than three corners is split into a fan of triangles from its first corner,
  so a four-corner face becomes two triangles.
- Each triangle's colour is the `Kd` diffuse colour of its `usemtl` material in the MTL file.
  Without a material colour, the placement's `color` is used.
- Corners without a `vn` normal get the area-weighted average of the normals of the faces
  around their vertex, which shades curved surfaces smoothly.

The result is a `Mesh`: positions, three vertex indices per triangle, a normal per triangle
corner, and an optional colour per triangle.

## 2. Placing a model

Models are placed in `level.ts` with `model()` from `levelBuilder.ts`:

```ts
model({ mesh: peasant, x: 2.4, y: 4.2, angle: 270, height: 0.8 }),
```

| Option | Meaning |
|---|---|
| `x`, `y` | Centre of the model's footprint on the map, in world units (one cell is 1 by 1) |
| `z` | Height of the model's lowest point; default 0, the ground |
| `angle` | Degrees around the vertical axis; 0 faces the file's front (+Z) towards +y |
| `height` | Scale the model to exactly this tall; when unset, `scale` is used |
| `scale` | Uniform scale; default 1 |
| `color` | Colour for triangles without a material colour; default light grey |

Models are drawn only. They do not block movement.

## 3. Baking: `compileLevel`

When the level compiles, `bakeModel` turns every placed model into world-space triangles:

1. **Axes.** OBJ files are Y up; the world is Z up with the ground on x/y. A file point
   (x, y, z) becomes world (x, z, y), and normals get the same swap.
2. **Turn** around the vertical axis by `angle`.
3. **Scale** so the model is `height` tall, or by `scale`.
4. **Position** so the footprint's centre is at `x`, `y` and the lowest point is at `z`.

Each triangle is written as **21 floats** into one `Float32Array`, `meshTriangles`:

```
corner 1:  x y z  nx ny nz      world position and unit normal
corner 2:  x y z  nx ny nz
corner 3:  x y z  nx ny nz
colour:    r g b                0-1
```

`MESH_TRIANGLE_STRIDE = 21` in `lib.rs` and `levelBuilder.ts` must match this layout.

`compileLevel` also outputs a **model table**, `models`, with 6 floats per model:

```
first triangle, triangle count, footprint min x, min y, max x, max y
```

## 4. Upload and registration (once, in `Camera`)

- `meshTriangles` is copied into WASM memory once (`meshRef`) and passed to `render` each frame
  as a pointer and length.
- `populateModels(models)` registers each model in `WasmStripePerCoordMap`, the same per-cell
  registry the billboard sprites use. A model is listed in every grid cell its footprint
  touches. A footprint ending exactly on a cell line does not reach the next cell.

## 5. Which models are drawn: per-cell visibility

Models use the sprites' visibility mechanism:

1. `raycast_column` casts one ray per screen column through the grid. On every 5th column, it
   records each cell the ray passes through that holds a sprite or a model
   (`WasmStripePerCoordMap::occupied`).
2. Rays stop at walls. Windows and doors let them continue.
3. `draw_walls_raycast` collects the sprites and model ids from all recorded cells. A model in
   several reached cells is listed once.
4. `render` slices each found model's triangle range out of the uploaded data and passes only
   those runs to `draw_meshes`.

A model behind the camera, outside the view, or in a room hidden behind walls is never
recorded, so it costs no per-triangle work. As with sprites, a distant model whose cell is
only crossed by the columns between the sampled ones can be missed.

## 6. Frame order and the shared depth buffer

`render` runs the passes in this order. `depth` (`depthRef` in `Camera`) holds one float per
screen pixel: the distance of the nearest floor, ceiling, roof or model drawn there so far.

| Pass | Writes colour | Writes depth | Checks depth |
|---|---|---|---|
| Sky (`draw_background_image_prescaled`) | every pixel | no | no |
| Floor, ceiling and roof (`draw_ceiling_floor_raycast`) | its pixels | resets every row to infinity, then stores surface depth | no |
| Models (`draw_meshes`) | its pixels | yes | skips pixels behind floors, roofs and nearer triangles |
| Walls, doors, windows, billboards (`draw_sprites_wasm`) | its pixels | no | skips pixels behind floors, roofs and models |

Colour and depth are written together, pixel by pixel. No separate model image exists; a
model's pixels appear in the frame one at a time among everything else, and at every moment a
pixel's colour and its depth entry describe the same thing.

The sprite pass draws farthest first, so it needs no depth writes of its own: nearer walls
and billboards simply paint over farther ones. Window glass is semi-transparent and blends
with whatever is already in the frame, so glass in front of a model shows the model behind it.

## 7. Distance units

Every depth in the buffer is distance **along the view direction**, not straight-line
distance, on the same scale `translate_coordinate_to_camera` uses for sprites:

```
scale    = |plane_x * dir_y - dir_x * plane_y| / plane_y_initial
sideways = scale * ( dir_y   * dx - dir_x   * dy)      dx, dy: point minus player position
depth    = scale * (-plane_y * dx + plane_x * dy)
```

The floor pass computes its row depths on the same scale. This shared scale is what lets the
sprite pass compare a wall stripe's distance with a model pixel's distance. The
`quad_matches_sprite_projection` test checks that a mesh and a sprite at the same place get
the same depth to within 0.001.

## 8. Triangle setup (once per triangle per frame, in parallel)

`draw_meshes` steps through each visible run 21 floats at a time. Each triangle:

1. **Camera space.** Each corner becomes (sideways, depth, height) with the formulas above,
   keeping its world height z.
2. **Lighting per corner.** `light = 0.35 + 0.65 * |n · sun|` with the fixed sun direction
   (0.4, 0.3, 0.866). The absolute value lights both sides, so winding order does not matter.
3. **Culling.** The triangle is skipped when:
   - all corners are closer than `MESH_NEAR_DEPTH` (0.05), i.e. behind the eye;
   - all corners have `sideways > depth` (beyond the right edge) or all `sideways < -depth`
     (beyond the left edge). Each of these is a half-space through the eye that contains no
     visible pixel;
   - it faces away from the eye (`faces_away_from`). The geometric face normal is oriented to
     agree with the vertex normals, which decide which side is outside; the triangle is culled
     when that outside points away from the eye. On a closed model such faces are always
     covered by its near side.
4. **Near-plane clipping.** If the triangle crosses depth 0.05, the part in front of the eye
   is cut away (`clip_to_near_plane`). Position and light are interpolated along the cut
   edges. The result has 3 or 4 corners; 4 corners are split into two triangles.
5. **Distance darkening.** Like sprites, the colour is multiplied by
   `clamp(1 - (average depth / light_range - map_light), 0.125, 1)`.
6. **Projection.**
   ```
   half_width    = width / 2
   middle_view_y = height / 2 + pitch
   eye           = player z + 0.5
   screen_x      = half_width * (1 + sideways / depth)
   screen_y      = middle_view_y + (half_width / depth) * (eye - z)
   ```
   Each corner also keeps `1 / depth` and `light / depth`. Depth itself does not change
   linearly across the screen, but `1 / depth` does, and so does `light / depth`. That is what
   makes depth and shading correct on surfaces seen at an angle.
7. **Screen planes.** For the triangle's three edges, and for `1 / depth` and `light / depth`,
   the setup stores a plane `a * x + b * y + c` over screen pixels:
   - the three edge planes are barycentric weights, each 0 on one edge and 1 at the opposite
     corner. A pixel is inside the triangle when all three are at least 0. Dividing by the
     triangle's signed area makes this work for either winding;
   - the depth and light planes give `1 / depth` and `light / depth` at any pixel with two
     multiplies and two adds.
   The setup also stores the triangle's screen bounding box and its nearest corner's
   `1 / depth`.

## 9. Ordering and banding

- All set-up triangles of all visible models are sorted **nearest first** by their nearest
  corner. Nearer triangles fill the depth buffer early, so pixels hidden behind them fail the
  depth test before any shading.
- The screen is split into bands of `MESH_BAND_ROWS` (8) rows. Each band gets the list of
  triangles whose bounding box overlaps it, in sorted order. One thread fills one band, so
  threads never write the same pixels.

## 10. Filling a triangle

For each row of a band that the triangle's bounding box covers:

1. **Row span.** Along a row, each edge plane is a straight line in x. Solving `weight >= 0`
   for each of the three gives the exact x range of pixel centres inside the triangle. Pixels
   outside it are never visited.
2. **Each pixel in the span:**
   ```
   inv_depth = plane(1 / depth) at the pixel centre
   if inv_depth * depth_buffer[pixel] <= 1:  skip     // something nearer is already there
   depth = 1 / inv_depth
   depth_buffer[pixel] = depth
   light = plane(light / depth) at the pixel centre * depth
   colour[pixel] = triangle colour * light
   ```
   The skip test is `depth >= depth_buffer[pixel]` rearranged into `1 / depth` terms, so hidden
   pixels need no division. An infinite buffer entry (nothing drawn yet) never hides anything.

## 11. Compositing with walls, windows and billboards

After the mesh pass, `draw_sprites_wasm` draws wall, door and window stripes (one pixel wide,
one per screen column per hit) and billboard sprites, farthest first. Each part has one depth
for its whole height, `projection.distance` from `translate_coordinate_to_camera`: the ray's
hit point for a stripe, the centre for a billboard. For every pixel it would cover:

```
if sprite.depth >= depth_buffer[pixel]: skip    // a floor, roof or model is nearer
```

Otherwise it writes over the model's colour; window glass blends with it. A wall, door or
window stripe whose every pixel is behind the buffer is skipped entirely.

A billboard has one depth for its whole height, so it behaves as a flat card at its centre's
distance: a model standing partly inside a tree is cut off sharply at that distance.

## Limits

- Models are coloured per triangle; textures are not sampled.
- Models do not block movement.
- Models are found through cells recorded on every 5th screen column, like sprites.
- Model text is bundled with `?raw` and parsed at startup.

## Tests

`src/tests/mesh.rs`:

- `quad_matches_sprite_projection`: top, bottom and depth match a sprite to within one pixel
  and 0.001 depth
- `nearer_floors_hide_the_mesh`: depth in the shared buffer hides mesh pixels
- `walls_and_sprites_composite_with_meshes_by_depth`: through the real sprite pass, glass in
  front blends over a mesh, an opaque wall in front covers it, and a mesh in front hides walls
  and windows
- `nearer_triangle_wins_regardless_of_order`
- `triangles_crossing_behind_the_camera_are_clipped`
- `model_footprints_register_every_cell_they_touch`
- `only_models_in_cells_the_rays_reach_are_found`: through the real raycaster, with a wall, a
  model behind the camera and a model spanning three cells
