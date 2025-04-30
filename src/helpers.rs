use js_sys::Float32Array;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, slice::from_raw_parts};
use wasm_bindgen::prelude::*;

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

#[wasm_bindgen]
pub struct WasmStripePerCoordMap {
    map: HashMap<(i32, i32), Vec<[f32; 5]>>,
}

// 🦀 Rust-only implementation block
impl WasmStripePerCoordMap {
    pub fn get_map(&self) -> &HashMap<(i32, i32), Vec<[f32; 5]>> {
        &self.map
    }
}

#[wasm_bindgen]
impl WasmStripePerCoordMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Accepts a JS Float32Array directly!
    #[wasm_bindgen(js_name = populateFromArray)]
    pub fn populate_from_array(&mut self, sprite_data: &[f32]) {
        let mut sprites_map: HashMap<(i32, i32), Vec<[f32; 5]>> = HashMap::new();

        for i in (0..sprite_data.len()).step_by(5) {
            let sprite: [f32; 5] = sprite_data[i..i + 5].try_into().unwrap();
            let key = (sprite[0].floor() as i32, sprite[1].floor() as i32);
            sprites_map.entry(key).or_default().push(sprite);
        }

        self.map = sprites_map;
    }

    #[wasm_bindgen]
    pub fn count_cells(&self) -> usize {
        self.map.len()
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
    pub dir_x: f32,
    pub dir_y: f32,
    pub plane_x: f32,
    pub plane_y: f32,
    pub pitch: i32,
    pub z: i32, // TODO: i32?
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
    pub x_fixed: i32,
    pub y_fixed: i32,
    pub angle: i32,
    pub height: i32,
    pub r#type: i32,
    pub column: u32,
    pub side: u8,
    pub offset: f32,
    pub width: f32,
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
    #[wasm_bindgen(constructor)]
    pub fn new(
        bg_img_texture: *const u8,
        texture_width: i32,
        texture_height: i32,
        screen_width: i32,
        screen_height: i32,
    ) -> BackgroundImageWasm {
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

        BackgroundImageWasm {
            data,
            width: sky_width,
            height: screen_height,
        }
    }
}
