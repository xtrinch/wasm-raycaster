use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Texture ids the render passes look up. The game's wasm TextureType mirrors these values.
pub enum TextureType {
    WALL = 1,
    CEILING = 2,
    FLOOR = 3,
    ROAD = 4,
    DOOR = 5,
    TREE_CONE = 6,
    PILLAR = 7,
    BUSH1 = 8,
    TREE_VASE = 9,
    TREE_COLUMNAR = 10,
    LADY = 11,
    WINDOW = 12,
    ROOF = 13,
}

// Each texture's size and number of view angles, by TextureType value.
pub struct TextureMetaMap {
    map: HashMap<i32, TextureData>,
}

#[derive(Clone)]
pub struct TextureData {
    pub width: i32,
    pub height: i32,
    pub angles: u32,
    // pub data: Vec<u8>,
}

impl TextureMetaMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key: i32, width: i32, height: i32, angles: u32) {
        self.map.insert(
            key,
            TextureData {
                width,
                height,
                angles, // data,
            },
        );
    }

    pub fn get_map(&self) -> &HashMap<i32, TextureData> {
        &self.map
    }

    pub fn get(&self, key: i32) -> Option<&TextureData> {
        self.map.get(&key)
    }
}

// RGBA texels by (TextureType value, view angle).
pub struct TextureMap {
    map: HashMap<(i32, i32), Vec<u8>>,
}

impl TextureMap {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(&mut self, key0: i32, angle: i32, sprite_data: &[u8]) {
        self.map.insert((key0, angle), sprite_data.to_vec());
    }

    pub fn get_map(&self) -> &HashMap<(i32, i32), Vec<u8>> {
        &self.map
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }
}

// A grid cell: world x and y rounded down. Cells may be negative, outside the grid.
pub type Cell = (i32, i32);

// Every cell a footprint touches; one ending exactly on a cell line does not reach the next.
pub fn footprint_cells(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Vec<Cell> {
    let (first_x, first_y) = (min_x.floor() as i32, min_y.floor() as i32);
    let last_x = (max_x.ceil() as i32 - 1).max(first_x);
    let last_y = (max_y.ceil() as i32 - 1).max(first_y);
    (first_y..=last_y)
        .flat_map(|y| (first_x..=last_x).map(move |x| (x, y)))
        .collect()
}

// An obstacle for movement in world units: an axis-aligned footprint and its top height.
// Walls, windows and solid models all collide as these boxes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CollisionBox {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
    pub top: f32,
}

// What blocks movement besides the walls and windows decoded from the grid: solid models,
// kept by the world under their ids.
pub struct CollisionCells {
    boxes: HashMap<u32, CollisionBox>,
    cells: HashMap<Cell, Vec<u32>>, // ids of the boxes touching each cell
}

impl CollisionCells {
    pub fn new() -> Self {
        Self {
            boxes: HashMap::new(),
            cells: HashMap::new(),
        }
    }

    // Registers the box in every cell it touches, replacing any box with the same id.
    pub fn insert(&mut self, id: u32, solid: CollisionBox) {
        self.remove(id);
        self.boxes.insert(id, solid);
        for cell in footprint_cells(solid.min_x, solid.min_y, solid.max_x, solid.max_y) {
            self.cells.entry(cell).or_default().push(id);
        }
    }

    pub fn remove(&mut self, id: u32) {
        let Some(solid) = self.boxes.remove(&id) else {
            return;
        };
        for cell in footprint_cells(solid.min_x, solid.min_y, solid.max_x, solid.max_y) {
            if let Some(ids) = self.cells.get_mut(&cell) {
                ids.retain(|&other| other != id);
                if ids.is_empty() {
                    self.cells.remove(&cell);
                }
            }
        }
    }

    // Each box touching the cells from first to last, inclusive, listed once in id order.
    pub fn boxes_in(&self, first: Cell, last: Cell) -> Vec<CollisionBox> {
        let mut ids: Vec<u32> = (first.1..=last.1)
            .flat_map(|y| (first.0..=last.0).map(move |x| (x, y)))
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| self.boxes[&id]).collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub map_x: i32,
    pub map_y: i32,
    pub dir_x: f32,
    pub dir_y: f32,
    pub plane_x: f32,
    pub plane_y: f32,
    pub pitch: i32,
    pub z: f32, // Feet height in world units, like x and y.
    pub plane_y_initial: f32,
}

// Data structures
#[derive(Serialize, Deserialize, Clone)]
pub struct Sprite {
    pub x: f32,
    pub y: f32,
    pub fract: f32, // offset inside the coord
    pub dx: f32,
    pub dy: f32,
    pub angle: i32,
    pub height: i32,
    pub r#type: i32,
    pub column: u32,
    pub distance: f32,
    pub distance_fixed: i32,
}

pub struct TranslationResult {
    pub screen_x: i32,
    pub screen_y_ceiling: i32,
    pub distance: f32,
    pub full_height: i32,
}

#[derive(Serialize, Clone, Copy)]
pub struct SpritePart<'a> {
    pub depth: f32,
    pub sprite_type: i32, // TODO: u8?
    pub sprite_left_x: u32,
    pub width: i32,
    pub screen_y_ceiling: i32,
    pub height: i32,
    pub tex_x1: i32,
    pub tex_width: i32,
    pub alpha: i32,
    pub angle: i32,
    pub full_texture_height: i32,
    pub full_texture_width: i32,
    pub full_texture_data: &'a Vec<u8>,
    pub inv_sprite_width: i32,
    pub inv_sprite_height: i32,
}

// A mesh triangle projected to the screen, ready to rasterize. Each [a, b, c] is a screen
// plane a * x + b * y + c evaluated at pixel centres.
#[derive(Clone, Copy)]
pub struct ScreenTriangle {
    pub weights: [[f32; 3]; 3], // barycentric weights, each >= 0 inside the triangle
    pub inv_depth: [f32; 3],    // 1 / camera depth, which interpolates linearly on screen
    pub shade: [f32; 3],        // light level / camera depth, divided back per pixel
    pub color: [f32; 3],        // 0-255, already darkened by distance
    pub nearest: f32,           // largest 1 / depth of the corners, for front-to-back order
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

#[inline(always)]
pub fn has_bit_set(value: u64, bit: u8) -> bool {
    (value & (1 << bit)) != 0
}

#[inline(always)]
pub fn get_grid_value(map_x: i32, map_y: i32, map_width: i32, map_data: &[u64]) -> u64 {
    if map_x < 0 || map_y < 0 || map_x >= map_width || map_y >= map_width {
        return 0;
    }

    let map_index = (map_y * map_width + map_x) as usize;
    if map_index >= map_data.len() {
        return 0;
    }

    return map_data[map_index];
}

#[inline(always)]
pub fn get_bits(value: u64, start_bit: u8) -> u8 {
    ((value >> start_bit) & 0b1111) as u8
}

pub struct Texture<'a> {
    pub data: &'a [u8],
    pub width: i32,
    pub height: i32,
}

pub const FIXED_SHIFT: usize = 20;
pub const FIXED_SHIFT_LARGE: usize = 8;
pub const FIXED_ONE: i32 = 1 << FIXED_SHIFT;
pub const FIXED_ONE_LARGE: i32 = 1 << FIXED_SHIFT_LARGE;

#[inline(always)]
pub fn to_fixed(f: f32) -> i32 {
    (f * (FIXED_ONE as f32)) as i32
}

// Keep distant floor coordinates from overflowing the 32-bit fixed-point range.
#[inline(always)]
pub fn to_fixed_i64(f: f32) -> i64 {
    (f * (FIXED_ONE as f32)) as i64
}

#[inline(always)]
pub fn to_fixed_large(f: f32) -> i32 {
    (f * (FIXED_ONE_LARGE as f32)) as i32
}

#[inline(always)]
pub fn fixed_mul(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> FIXED_SHIFT) as i32
}

#[inline(always)]
pub fn fixed_div(a: i32, b: i32) -> i32 {
    if b == 0 {
        0
    } else {
        ((a as i64) << FIXED_SHIFT) as i32 / b
    }
}

#[inline]
pub fn from_fixed_to_f32(x: i32) -> f32 {
    x as f32 / (1 << FIXED_SHIFT) as f32
}

// The sky panorama, prescaled to the screen height and twice its width.
pub struct Background {
    data: Vec<u8>,
    width: i32,
    height: i32,
}

impl Background {
    pub fn new() -> Background {
        Background {
            data: vec![],
            width: 0,
            height: 0,
        }
    }

    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }

    // Prescales an RGBA panorama of texture_width by texture_height pixels.
    pub fn set(
        &mut self,
        src: &[u8],
        texture_width: i32,
        texture_height: i32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let sky_scale = screen_height as f64 / texture_height as f64;
        let sky_width = (texture_width as f64 * sky_scale * 2.0).round() as i32;

        let mut data = vec![0u8; (sky_width * screen_height * 4) as usize];

        for y in 0..screen_height {
            let src_y = (y * texture_height / screen_height).clamp(0, texture_height - 1);
            for x in 0..sky_width {
                let src_x = ((x * texture_width) / sky_width) % texture_width;

                let src_idx = ((src_y * texture_width + src_x) * 4) as usize;
                let dst_idx = ((y * sky_width + x) * 4) as usize;

                data[dst_idx..dst_idx + 3].copy_from_slice(&src[src_idx..src_idx + 3]);
                data[dst_idx + 3] = 255;
            }
        }

        self.data = data;
        self.height = screen_height;
        self.width = sky_width;
    }
}
