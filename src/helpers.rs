use js_sys::Float32Array;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, slice::from_raw_parts};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
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

#[wasm_bindgen]
pub struct WasmTextureMetaMap {
    map: HashMap<i32, TextureData>,
}

#[derive(Clone)]
pub struct TextureData {
    pub width: i32,
    pub height: i32,
    pub angles: u32,
    // pub data: Vec<u8>,
}

#[wasm_bindgen]
impl WasmTextureMetaMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    #[wasm_bindgen(js_name = populateFromArray)]
    pub fn populate_from_array(&mut self, key: i32, width: i32, height: i32, angles: u32) {
        self.map.insert(
            key,
            TextureData {
                width,
                height,
                angles, // data,
            },
        );
    }
}

impl WasmTextureMetaMap {
    pub fn get_map(&self) -> &HashMap<i32, TextureData> {
        &self.map
    }

    pub fn get(&self, key: i32) -> Option<&TextureData> {
        self.map.get(&key)
    }
}

#[wasm_bindgen]
pub struct WasmTextureMap {
    map: HashMap<(i32, i32), Vec<u8>>,
}

impl WasmTextureMap {
    pub fn get_map(&self) -> &HashMap<(i32, i32), Vec<u8>> {
        &self.map
    }
}

#[wasm_bindgen]
impl WasmTextureMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    #[wasm_bindgen(js_name = populateFromArray)]
    pub fn populate_from_array(&mut self, key0: i32, angle: i32, sprite_data: &[u8]) {
        self.map.insert((key0, angle), sprite_data.to_vec());
    }

    #[wasm_bindgen]
    pub fn count_cells(&self) -> usize {
        self.map.len()
    }
}

// A grid cell: world x and y rounded down. Cells may be negative, outside the grid.
pub type Cell = (i32, i32);

// Every cell a footprint touches; one ending exactly on a cell line does not reach the next.
fn footprint_cells(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Vec<Cell> {
    let (first_x, first_y) = (min_x.floor() as i32, min_y.floor() as i32);
    let last_x = (max_x.ceil() as i32 - 1).max(first_x);
    let last_y = (max_y.ceil() as i32 - 1).max(first_y);
    (first_y..=last_y)
        .flat_map(|y| (first_x..=last_x).map(move |x| (x, y)))
        .collect()
}

// A model's triangles in the uploaded mesh buffer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshRange {
    pub first_triangle: usize,
    pub triangle_count: usize,
}

// What each cell holds for drawing. Rays record the cells they pass through that hold
// anything, and the billboards and models in those cells are drawn.
#[wasm_bindgen]
pub struct WasmRenderCells {
    billboards: HashMap<Cell, Vec<Sprite>>, // flat sprites standing in each cell
    model_meshes: Vec<MeshRange>,           // each model's triangles, indexed by model id
    models: HashMap<Cell, Vec<u32>>,        // ids of the models whose footprint touches each cell
}

// 🦀 Rust-only implementation block
impl WasmRenderCells {
    pub fn billboards_at(&self, cell: &Cell) -> &[Sprite] {
        self.billboards
            .get(cell)
            .map_or(&[], |sprites| sprites.as_slice())
    }

    pub fn models_at(&self, cell: &Cell) -> &[u32] {
        self.models
            .get(cell)
            .map_or(&[], |models| models.as_slice())
    }

    pub fn model_mesh(&self, model: u32) -> Option<MeshRange> {
        self.model_meshes.get(model as usize).copied()
    }

    // Whether a billboard or a model occupies the cell, so rays should record it.
    pub fn occupied(&self, cell: &Cell) -> bool {
        self.billboards.contains_key(cell) || self.models.contains_key(cell)
    }
}

#[wasm_bindgen]
impl WasmRenderCells {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            billboards: HashMap::new(),
            model_meshes: Vec::new(),
            models: HashMap::new(),
        }
    }

    /// Five floats per billboard: x, y, angle, height and texture type.
    #[wasm_bindgen(js_name = populateBillboards)]
    pub fn populate_billboards(&mut self, sprite_data: &[f32]) {
        self.billboards.clear();
        for sprite in sprite_data.chunks_exact(5) {
            let [x, y, angle, height, sprite_type] = sprite.try_into().unwrap();
            self.billboards
                .entry((x.floor() as i32, y.floor() as i32))
                .or_default()
                .push(Sprite {
                    // TODO: to a smaller struct
                    x,
                    y,
                    fract: 0., // offset inside the coord
                    dx: 0.,
                    dy: 0.,
                    angle: angle as i32,
                    height: height as i32,
                    r#type: sprite_type as i32,
                    column: 0,
                    distance: 0.,
                    distance_fixed: 0,
                });
        }
    }

    /// Six floats per model: first triangle, triangle count, then the footprint's
    /// min x, min y, max x, max y in world units. A model's id is its position in the
    /// data; it is registered in every cell the footprint touches.
    #[wasm_bindgen(js_name = populateModels)]
    pub fn populate_models(&mut self, model_data: &[f32]) {
        self.model_meshes.clear();
        self.models.clear();
        for (id, model) in model_data.chunks_exact(6).enumerate() {
            self.model_meshes.push(MeshRange {
                first_triangle: model[0] as usize,
                triangle_count: model[1] as usize,
            });
            for cell in footprint_cells(model[2], model[3], model[4], model[5]) {
                self.models.entry(cell).or_default().push(id as u32);
            }
        }
    }
}

// Floats per solid in populateSolids: footprint min x, min y, max x, max y, then top.
pub const SOLID_STRIDE: usize = 5;

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

// What blocks movement besides the walls and windows decoded from the grid.
#[wasm_bindgen]
pub struct WasmCollisionCells {
    boxes: Vec<CollisionBox>,       // each solid once, indexed by id
    cells: HashMap<Cell, Vec<u32>>, // ids of the boxes touching each cell
}

// 🦀 Rust-only implementation block
impl WasmCollisionCells {
    // Each box touching the cells from first to last, inclusive, listed once.
    pub fn boxes_in(&self, first: Cell, last: Cell) -> Vec<CollisionBox> {
        let mut ids: Vec<u32> = (first.1..=last.1)
            .flat_map(|y| (first.0..=last.0).map(move |x| (x, y)))
            .filter_map(|cell| self.cells.get(&cell))
            .flatten()
            .copied()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter().map(|id| self.boxes[id as usize]).collect()
    }
}

#[wasm_bindgen]
impl WasmCollisionCells {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            boxes: Vec::new(),
            cells: HashMap::new(),
        }
    }

    /// SOLID_STRIDE floats per solid: footprint min x, min y, max x, max y and top height
    /// in world units. A box's id is its position in the data; it is registered in every
    /// cell it touches.
    #[wasm_bindgen(js_name = populateSolids)]
    pub fn populate_solids(&mut self, solid_data: &[f32]) {
        self.boxes.clear();
        self.cells.clear();
        for (id, solid) in solid_data.chunks_exact(SOLID_STRIDE).enumerate() {
            self.boxes.push(CollisionBox {
                min_x: solid[0],
                min_y: solid[1],
                max_x: solid[2],
                max_y: solid[3],
                top: solid[4],
            });
            for cell in footprint_cells(solid[0], solid[1], solid[2], solid[3]) {
                self.cells.entry(cell).or_default().push(id as u32);
            }
        }
    }
}

#[wasm_bindgen]
pub struct WasmUint8Array(Vec<u8>);

#[wasm_bindgen]
impl WasmUint8Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    // set data from a JavaScript Uint8Array
    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Uint8Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmFloat32Array {
    buffer: Vec<f32>,
}

#[wasm_bindgen]
impl WasmFloat32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
        }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> Float32Array {
        unsafe { Float32Array::view_mut_raw(self.buffer.as_mut_ptr(), self.buffer.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut f32 {
        self.buffer.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: Float32Array) {
        let len = self.buffer.len().min(data.length() as usize);
        self.buffer[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmInt32Array(Vec<i32>);

#[wasm_bindgen]
impl WasmInt32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Int32Array {
        unsafe { js_sys::Int32Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut i32 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Int32Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmUInt32Array(Vec<u32>);

#[wasm_bindgen]
impl WasmUInt32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Uint32Array {
        unsafe { js_sys::Uint32Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u32 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Uint32Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmUInt64Array(Vec<u64>);

#[wasm_bindgen]
impl WasmUInt64Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::BigUint64Array {
        unsafe { js_sys::BigUint64Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u64 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::BigUint64Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

pub fn copy_to_raw_pointer<T: Copy>(ptr: *mut T, index: usize, data: &[T]) {
    unsafe {
        let target_ptr = ptr.add(index);
        for (i, &value) in data.iter().enumerate() {
            *target_ptr.add(i) = value;
        }
    }
}

use core::slice;

pub fn parse_sprite_texture_array(ptr: *const i32, len: usize) -> HashMap<i32, (i32, i32, i32)> {
    let mut map = HashMap::new();

    // Convert raw pointer to a safe slice
    let data: &[i32] = unsafe { slice::from_raw_parts(ptr, len) };

    // Process chunks of 3 (type, height, width)
    for chunk in data.chunks(4) {
        if let [sprite_type, height, width, angles] = *chunk {
            map.insert(sprite_type, (height, width, angles));
        }
    }

    map
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[wasm_bindgen]
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
#[derive(Serialize, Deserialize)]
#[wasm_bindgen]
pub struct Coords {
    pub x: i32,
    pub y: i32,
}

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

#[wasm_bindgen]
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

#[wasm_bindgen]
pub struct BackgroundImageWasm {
    data: Vec<u8>,
    width: i32,
    height: i32,
}

// 🦀 Rust-only implementation block
impl BackgroundImageWasm {
    pub fn get_data(&self) -> &Vec<u8> {
        &self.data
    }

    pub fn get_width(&self) -> i32 {
        self.width
    }
}

#[wasm_bindgen]
impl BackgroundImageWasm {
    #[wasm_bindgen]
    pub fn set(
        &mut self,
        bg_img_texture: *const u8,
        texture_width: i32,
        texture_height: i32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let src = unsafe {
            from_raw_parts(
                bg_img_texture,
                (texture_width * texture_height * 4) as usize,
            )
        };

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

    #[wasm_bindgen(constructor)]
    pub fn new() -> BackgroundImageWasm {
        BackgroundImageWasm {
            data: vec![],
            width: 0,
            height: 0,
        }
    }
}
