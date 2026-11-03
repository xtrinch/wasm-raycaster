// The level, kept in Rust: areas, walls, doors, windows, pillars, sprites and models, each
// added with an id so it can be removed or moved while the game runs. Every edit re-encodes
// only the cells it touches into the packed grid that rendering and movement read; the bit
// layout is documented in gridMap.ts. Positions are world units (one cell is 1 by 1);
// wall, door and window geometry must use multiples of 0.1, the grid's precision.

use crate::helpers::{footprint_cells, Cell, CollisionBox, CollisionCells, Sprite};
use serde::Deserialize;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::f64::consts::PI;

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SegmentKind {
    Wall,
    Door,
    Window,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    East,  // runs along y
    North, // runs along x
}

// Floor, ceiling/roof and road flags for whole cells; later areas replace earlier flags.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Area {
    pub x: i32, // whole cells
    pub y: i32,
    pub w: i32,
    pub h: i32,
    #[serde(default)]
    pub floor: bool,
    #[serde(default)]
    pub ceiling: bool,
    #[serde(default)]
    pub road: bool,
}

// A wall, door or window; pillars are square walls.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Segment {
    pub kind: SegmentKind,
    pub direction: Direction,
    pub x: f64, // low corner; thickness extends towards +x (east) or +y (north)
    pub y: f64,
    pub length: f64,
    pub thickness: f64,
}

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Billboard {
    pub x: f64,
    pub y: f64,
    pub texture: i32, // TextureType
    pub height: f64,  // multiplier of one wall height
    pub angle: f64,   // 0-360
}

// A placed mesh; it blocks movement only when solid.
#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Model {
    pub mesh: u32, // id from add_mesh
    pub x: f64,    // centre of the model's footprint
    pub y: f64,
    pub z: f64,              // height of the model's lowest point
    pub scale: f64,          // ignored when height is set
    pub height: Option<f64>, // scale the model to exactly this tall, in world units
    pub angle: f64, // degrees around the vertical axis; 0 faces the file's front (+Z) towards +y
    pub color: [f64; 3], // 0-1, for triangles without a material colour
    pub solid: bool, // block movement with the model's bounding box; feet can stand on its top
}

// The placements built by levelBuilder.ts, tagged by their "type" field.
#[derive(Deserialize, Clone, Copy, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Placement {
    Area(Area),
    Segment(Segment),
    Sprite(Billboard),
    Model(Model),
}

// A model's geometry in its file's own axes (OBJ is usually Y up).
pub struct Mesh {
    pub positions: Vec<f64>, // x, y, z per vertex
    pub triangles: Vec<u32>, // three vertex indices per triangle
    pub normals: Vec<f64>,   // x, y, z per triangle corner, aligned with triangles
    pub colors: Vec<f32>,    // r, g, b per triangle; NaN without a material colour
}

// Floats per baked mesh triangle, matching MESH_TRIANGLE_STRIDE in lib.rs.
const TRIANGLE_FLOATS: usize = 21;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Flags {
    floor: bool,
    ceiling: bool,
    road: bool,
}

// One segment's part inside one cell, in tenths of a cell; what a slot of the cell stores.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Piece {
    id: u32, // the segment it belongs to
    kind: SegmentKind,
    direction: Direction,
    offset: u64,    // across the segment, from the cell's low x/y edge
    thickness: u64, // size across the segment
    length: u64,    // size along the segment
    start: u64,     // along the segment, from the cell's low x/y edge
}

#[derive(Clone, Default)]
struct CellState {
    flags: Flags,
    pieces: SmallVec<[Piece; 3]>, // in the order they were added
}

#[derive(Default)]
struct CellBillboards {
    sprites: Vec<Sprite>,
    ids: Vec<u32>, // aligned with sprites
}

// A model with its triangles in world space, ready to draw.
struct PlacedModel {
    model: Model,
    triangles: Vec<f32>,
    footprint: [f32; 4], // min x, min y, max x, max y
    top: f32,
}

enum Item {
    Segment { cells: SmallVec<[usize; 4]> }, // indexes of the cells holding its pieces
    Billboard { cell: Cell },
    Model(PlacedModel),
}

pub struct World {
    size: usize,
    grid: Vec<u64>, // packed cells, row-major
    cells: Vec<CellState>,
    items: HashMap<u32, Item>,
    next_id: u32,
    meshes: Vec<Mesh>,
    billboards: HashMap<Cell, CellBillboards>, // sprites standing in each cell
    models: HashMap<Cell, Vec<u32>>, // ids of the models whose footprint touches each cell
    solids: CollisionCells,          // boxes of solid models, by model id
}

impl World {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            grid: vec![0; size * size],
            cells: vec![CellState::default(); size * size],
            items: HashMap::new(),
            next_id: 1,
            meshes: Vec::new(),
            billboards: HashMap::new(),
            models: HashMap::new(),
            solids: CollisionCells::new(),
        }
    }

    // Returns the new item's id, or 0 for areas, which only set flags and cannot be removed.
    pub fn add(&mut self, placement: Placement) -> Result<u32, String> {
        match placement {
            Placement::Area(area) => self.set_area(area).map(|_| 0),
            Placement::Segment(segment) => self.add_segment(segment),
            Placement::Sprite(billboard) => Ok(self.add_billboard(billboard)),
            Placement::Model(model) => self.add_model(model),
        }
    }

    pub fn set_area(&mut self, area: Area) -> Result<(), String> {
        let mut indexes = Vec::new();
        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                indexes.push(self.index(x as i64, y as i64, "area")?);
            }
        }
        let flags = Flags {
            floor: area.floor,
            ceiling: area.ceiling,
            road: area.road,
        };
        for index in indexes {
            self.cells[index].flags = flags;
            self.encode(index);
        }
        Ok(())
    }

    // Splits the segment into the cells it crosses. Nothing changes unless every cell it
    // touches can still hold its segments.
    pub fn add_segment(&mut self, segment: Segment) -> Result<u32, String> {
        let what = format!(
            "{} at ({}, {})",
            match segment.kind {
                SegmentKind::Wall => "wall",
                SegmentKind::Door => "door",
                SegmentKind::Window => "window",
            },
            segment.x,
            segment.y
        );
        let east = segment.direction == Direction::East;
        let position = format!("{what} position");
        let across = tenths(if east { segment.x } else { segment.y }, &position)?;
        let along = tenths(if east { segment.y } else { segment.x }, &position)?;
        let length = tenths(segment.length, &format!("{what} length"))?;
        let thickness = tenths(segment.thickness, &format!("{what} thickness"))?;
        if length <= 0 {
            return Err(format!("{what} needs a positive length"));
        }

        // A flat segment on a cell line goes into the cell after the line.
        let across_pieces = if thickness > 0 {
            split_range(across, thickness)
        } else {
            let cell = across.div_euclid(10);
            vec![(cell, across - cell * 10, 0)]
        };

        let id = self.next_id;
        let mut placed: Vec<(usize, Piece)> = Vec::new();
        for &(across_cell, offset, size) in &across_pieces {
            for (along_cell, start, piece_length) in split_range(along, length) {
                let (x, y) = if east {
                    (across_cell, along_cell)
                } else {
                    (along_cell, across_cell)
                };
                let index = self.index(x, y, &what)?;
                placed.push((
                    index,
                    Piece {
                        id,
                        kind: segment.kind,
                        direction: segment.direction,
                        offset: offset as u64,
                        thickness: size as u64,
                        length: piece_length as u64,
                        start: start as u64,
                    },
                ));
            }
        }

        // Check every touched cell before changing any of them.
        let mut updates = Vec::with_capacity(placed.len());
        for &(index, piece) in &placed {
            let mut pieces = self.cells[index].pieces.clone();
            pieces.push(piece);
            let where_ = format!("cell ({}, {})", index % self.size, index / self.size);
            if pieces.len() > 3 {
                return Err(format!(
                    "{where_} needs {} segments; a cell holds at most 3",
                    pieces.len()
                ));
            }
            let value = encode_any_order(self.cells[index].flags, &pieces)
                .map_err(|error| format!("{where_}: {error}"))?;
            updates.push((index, pieces, value));
        }

        for (index, pieces, value) in updates {
            self.cells[index].pieces = pieces;
            self.grid[index] = value;
        }
        self.next_id += 1;
        self.items.insert(
            id,
            Item::Segment {
                cells: placed.iter().map(|&(index, _)| index).collect(),
            },
        );
        Ok(id)
    }

    // Sprites may stand outside the grid.
    pub fn add_billboard(&mut self, billboard: Billboard) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let sprite = Sprite {
            x: billboard.x as f32,
            y: billboard.y as f32,
            fract: 0.,
            dx: 0.,
            dy: 0.,
            angle: billboard.angle as f32 as i32,
            // Sprite heights are percentages of one wall height.
            height: (billboard.height * 100.0) as f32 as i32,
            r#type: billboard.texture,
            column: 0,
            distance: 0.,
            distance_fixed: 0,
        };
        let cell = self.place_sprite(id, sprite);
        self.items.insert(id, Item::Billboard { cell });
        id
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> u32 {
        self.meshes.push(mesh);
        (self.meshes.len() - 1) as u32
    }

    pub fn add_model(&mut self, model: Model) -> Result<u32, String> {
        let mesh = self
            .meshes
            .get(model.mesh as usize)
            .ok_or_else(|| format!("model mesh {} was never added", model.mesh))?;
        let placed = bake_model(mesh, model);
        let id = self.next_id;
        self.next_id += 1;
        self.register_model(id, &placed);
        self.items.insert(id, Item::Model(placed));
        Ok(id)
    }

    // Removes any placement but an area; false if the id is unknown.
    pub fn remove(&mut self, id: u32) -> bool {
        let Some(item) = self.items.remove(&id) else {
            return false;
        };
        match item {
            Item::Segment { cells } => {
                for index in cells {
                    self.cells[index].pieces.retain(|piece| piece.id != id);
                    self.encode(index);
                }
            }
            Item::Billboard { cell } => self.unplace_sprite(id, cell),
            Item::Model(placed) => self.unregister_model(id, &placed),
        }
        true
    }

    // Moves a sprite or model to world x/y; false for walls, doors, windows or unknown ids.
    pub fn move_to(&mut self, id: u32, x: f64, y: f64) -> bool {
        match self.items.get(&id) {
            Some(&Item::Billboard { cell }) => {
                let index = self.billboards[&cell].ids.iter().position(|&i| i == id);
                let mut sprite = self.billboards[&cell].sprites[index.unwrap()].clone();
                self.unplace_sprite(id, cell);
                sprite.x = x as f32;
                sprite.y = y as f32;
                let cell = self.place_sprite(id, sprite);
                self.items.insert(id, Item::Billboard { cell });
                true
            }
            Some(Item::Model(placed)) => {
                let model = Model {
                    x,
                    y,
                    ..placed.model
                };
                let Some(Item::Model(old)) = self.items.remove(&id) else {
                    unreachable!()
                };
                self.unregister_model(id, &old);
                let placed = bake_model(&self.meshes[model.mesh as usize], model);
                self.register_model(id, &placed);
                self.items.insert(id, Item::Model(placed));
                true
            }
            _ => false,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn grid(&self) -> &[u64] {
        &self.grid
    }

    pub fn billboards_at(&self, cell: &Cell) -> &[Sprite] {
        self.billboards
            .get(cell)
            .map_or(&[], |billboards| billboards.sprites.as_slice())
    }

    pub fn models_at(&self, cell: &Cell) -> &[u32] {
        self.models
            .get(cell)
            .map_or(&[], |models| models.as_slice())
    }

    // Whether a sprite or a model occupies the cell, so rays should record it.
    pub fn occupied(&self, cell: &Cell) -> bool {
        self.billboards.contains_key(cell) || self.models.contains_key(cell)
    }

    // A model's world-space triangles, MESH_TRIANGLE_STRIDE floats each.
    pub fn model_triangles(&self, id: u32) -> Option<&[f32]> {
        match self.items.get(&id) {
            Some(Item::Model(placed)) => Some(&placed.triangles),
            _ => None,
        }
    }

    pub fn solids(&self) -> &CollisionCells {
        &self.solids
    }

    fn index(&self, x: i64, y: i64, what: &str) -> Result<usize, String> {
        let size = self.size as i64;
        if x < 0 || y < 0 || x >= size || y >= size {
            return Err(format!(
                "{what} reaches cell ({x}, {y}) outside the {size}x{size} grid"
            ));
        }
        Ok((y * size + x) as usize)
    }

    // Re-packs a cell whose flags changed or which lost a segment. Any one or two segments
    // fit a cell, so a cell that held its segments before always holds them after.
    fn encode(&mut self, index: usize) {
        let cell = &self.cells[index];
        self.grid[index] = encode_any_order(cell.flags, &cell.pieces)
            .expect("a cell's remaining segments always fit");
    }

    fn place_sprite(&mut self, id: u32, sprite: Sprite) -> Cell {
        let cell = (sprite.x.floor() as i32, sprite.y.floor() as i32);
        let billboards = self.billboards.entry(cell).or_default();
        billboards.sprites.push(sprite);
        billboards.ids.push(id);
        cell
    }

    fn unplace_sprite(&mut self, id: u32, cell: Cell) {
        let billboards = self.billboards.get_mut(&cell).unwrap();
        let index = billboards.ids.iter().position(|&i| i == id).unwrap();
        billboards.sprites.remove(index);
        billboards.ids.remove(index);
        if billboards.ids.is_empty() {
            self.billboards.remove(&cell);
        }
    }

    fn register_model(&mut self, id: u32, placed: &PlacedModel) {
        let [min_x, min_y, max_x, max_y] = placed.footprint;
        for cell in footprint_cells(min_x, min_y, max_x, max_y) {
            self.models.entry(cell).or_default().push(id);
        }
        if placed.model.solid {
            self.solids.insert(
                id,
                CollisionBox {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    top: placed.top,
                },
            );
        }
    }

    fn unregister_model(&mut self, id: u32, placed: &PlacedModel) {
        let [min_x, min_y, max_x, max_y] = placed.footprint;
        for cell in footprint_cells(min_x, min_y, max_x, max_y) {
            if let Some(ids) = self.models.get_mut(&cell) {
                ids.retain(|&other| other != id);
                if ids.is_empty() {
                    self.models.remove(&cell);
                }
            }
        }
        self.solids.remove(id);
    }
}

fn tenths(value: f64, name: &str) -> Result<i64, String> {
    let scaled = (value * 10.0).round();
    if (value * 10.0 - scaled).abs() > 1e-6 {
        return Err(format!("{name} {value} is not a multiple of 0.1"));
    }
    Ok(scaled as i64)
}

// Clips [start, start + size) in tenths to each cell it touches: cell, offset in the cell
// and size in the cell.
fn split_range(start: i64, size: i64) -> Vec<(i64, i64, i64)> {
    let mut pieces = Vec::new();
    let mut cell = start.div_euclid(10);
    while cell * 10 < start + size {
        let from = start.max(cell * 10);
        let to = (start + size).min(cell * 10 + 10);
        pieces.push((cell, from - cell * 10, to - from));
        cell += 1;
    }
    pieces
}

const SLOT_DOOR_BITS: [u64; 3] = [5, 4, 4];
const SLOT_DIRECTION_BITS: [u64; 3] = [6, 7, 8];
const SLOT_WINDOW_BITS: [u64; 2] = [9, 10];
const SEGMENT_COUNT_SHIFT: u64 = 12;
const SLOT_SHIFTS: [u64; 3] = [16, 32, 48];

// Packs a cell with its segments in slot order; see gridMap.ts for the layout.
fn encode_cell(flags: Flags, pieces: &[Piece]) -> Result<u64, &'static str> {
    if pieces.len() == 3
        && (pieces[1].kind == SegmentKind::Door) != (pieces[2].kind == SegmentKind::Door)
    {
        return Err("Segments 1 and 2 share one door bit; both or neither must be doors");
    }
    let mut value = 0u64;
    // Bit 0 marks cells with segments; the minimap colours them by it.
    if !pieces.is_empty() {
        value |= 1;
    }
    value |= (flags.floor as u64) << 1 | (flags.ceiling as u64) << 2 | (flags.road as u64) << 3;
    value |= (pieces.len() as u64) << SEGMENT_COUNT_SHIFT;
    for (slot, piece) in pieces.iter().enumerate() {
        match piece.kind {
            SegmentKind::Door => value |= 1 << SLOT_DOOR_BITS[slot],
            SegmentKind::Window => match SLOT_WINDOW_BITS.get(slot) {
                Some(bit) => value |= 1 << bit,
                None => return Err("Only segments 0 and 1 can be windows"),
            },
            SegmentKind::Wall => {}
        }
        if piece.direction == Direction::North {
            value |= 1 << SLOT_DIRECTION_BITS[slot];
        }
        let shift = SLOT_SHIFTS[slot];
        value |= piece.offset << shift
            | piece.thickness << (shift + 4)
            | piece.length << (shift + 8)
            | piece.start << (shift + 12);
    }
    Ok(value)
}

// Slot order matters for windows and the shared door bit, so try each order of the
// segments, in the order they were added first; the last error wins if none fits.
fn encode_any_order(flags: Flags, pieces: &[Piece]) -> Result<u64, &'static str> {
    let mut last_error = "a cell holds at most 3 segments";
    for order in permutations(pieces.len()) {
        let ordered: SmallVec<[Piece; 3]> = order.iter().map(|&i| pieces[i]).collect();
        match encode_cell(flags, &ordered) {
            Ok(value) => return Ok(value),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}

// Every order of 0..count, each starting with its lowest remaining index first.
fn permutations(count: usize) -> Vec<Vec<usize>> {
    fn of(items: &[usize]) -> Vec<Vec<usize>> {
        if items.len() <= 1 {
            return vec![items.to_vec()];
        }
        (0..items.len())
            .flat_map(|i| {
                let mut rest = items.to_vec();
                let item = rest.remove(i);
                of(&rest).into_iter().map(move |mut order| {
                    order.insert(0, item);
                    order
                })
            })
            .collect()
    }
    of(&(0..count).collect::<Vec<_>>())
}

// Places a mesh in the world: OBJ files are Y up and the world is Z up with the ground on
// x/y. The footprint is centred on the model's x/y and its lowest point stands on z.
fn bake_model(mesh: &Mesh, model: Model) -> PlacedModel {
    let angle = model.angle * PI / 180.0;
    let (cos, sin) = (angle.cos(), angle.sin());
    // Swap the file's Y-up axes to the world's Z-up, then turn around the vertical axis.
    let to_world = |x: f64, y: f64, z: f64| [x * cos - z * sin, x * sin + z * cos, y];

    let world: Vec<[f64; 3]> = mesh
        .positions
        .chunks_exact(3)
        .map(|p| to_world(p[0], p[1], p[2]))
        .collect();
    let mut min = [f64::INFINITY; 3];
    let mut max = [f64::NEG_INFINITY; 3];
    for vertex in &world {
        for k in 0..3 {
            min[k] = min[k].min(vertex[k]);
            max[k] = max[k].max(vertex[k]);
        }
    }
    let scale = match model.height {
        Some(height) => height / (max[2] - min[2]),
        None => model.scale,
    };
    let shift = [
        model.x - ((min[0] + max[0]) / 2.0) * scale,
        model.y - ((min[1] + max[1]) / 2.0) * scale,
        model.z - min[2] * scale,
    ];

    let mut triangles = Vec::with_capacity(mesh.triangles.len() / 3 * TRIANGLE_FLOATS);
    for (t, corners) in mesh.triangles.chunks_exact(3).enumerate() {
        for (k, &vertex) in corners.iter().enumerate() {
            let corner = t * 3 + k;
            let v = world[vertex as usize];
            let n = &mesh.normals[corner * 3..corner * 3 + 3];
            let normal = to_world(n[0], n[1], n[2]);
            triangles.extend((0..3).map(|i| (v[i] * scale + shift[i]) as f32));
            triangles.extend(normal.map(|c| c as f32));
        }
        let color = &mesh.colors[t * 3..t * 3 + 3];
        if color[0].is_nan() {
            triangles.extend(model.color.map(|c| c as f32));
        } else {
            triangles.extend_from_slice(color);
        }
    }

    let half_width = ((max[0] - min[0]) / 2.0) * scale;
    let half_depth = ((max[1] - min[1]) / 2.0) * scale;
    PlacedModel {
        model,
        triangles,
        footprint: [
            (model.x - half_width) as f32,
            (model.y - half_depth) as f32,
            (model.x + half_width) as f32,
            (model.y + half_depth) as f32,
        ],
        top: (model.z + (max[2] - min[2]) * scale) as f32,
    }
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod world_tests;
