// Placement types and the pure pieces of World: snapping to the grid's 0.1 precision,
// splitting segments into cells, packing cells (layout in gridMap.ts) and baking models.

use crate::helpers::{Cell, Sprite};
use serde::Deserialize;
use smallvec::SmallVec;
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
pub struct SpritePlacement {
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
    Sprite(SpritePlacement),
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
pub struct Flags {
    pub floor: bool,
    pub ceiling: bool,
    pub road: bool,
}

// One segment's part inside one cell, in tenths of a cell; what a slot of the cell stores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub id: u32, // the segment it belongs to
    pub kind: SegmentKind,
    pub direction: Direction,
    pub offset: u64,    // across the segment, from the cell's low x/y edge
    pub thickness: u64, // size across the segment
    pub length: u64,    // size along the segment
    pub start: u64,     // along the segment, from the cell's low x/y edge
}

#[derive(Clone, Default)]
pub struct CellState {
    pub flags: Flags,
    pub pieces: SmallVec<[Piece; 3]>, // in the order they were added
}

#[derive(Default)]
pub struct CellSprites {
    pub sprites: Vec<Sprite>,
    pub ids: Vec<u32>, // aligned with sprites
}

// A model with its triangles in world space, ready to draw.
pub struct PlacedModel {
    pub model: Model,
    pub triangles: Vec<f32>,
    pub footprint: [f32; 4], // min x, min y, max x, max y
    pub top: f32,
}

// Everything placed with an id.
pub enum Item {
    Segment { cells: SmallVec<[usize; 4]> }, // indexes of the cells holding its pieces
    Sprite { cell: Cell },
    Model(PlacedModel),
}

pub fn tenths(value: f64, name: &str) -> Result<i64, String> {
    let scaled = (value * 10.0).round();
    if (value * 10.0 - scaled).abs() > 1e-6 {
        return Err(format!("{name} {value} is not a multiple of 0.1"));
    }
    Ok(scaled as i64)
}

// Clips [start, start + size) in tenths to each cell it touches: cell, offset in the cell
// and size in the cell.
pub fn split_range(start: i64, size: i64) -> Vec<(i64, i64, i64)> {
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
pub fn encode_any_order(flags: Flags, pieces: &[Piece]) -> Result<u64, &'static str> {
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
pub fn bake_model(mesh: &Mesh, model: Model) -> PlacedModel {
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
