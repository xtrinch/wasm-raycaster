use js_sys::Float32Array;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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

pub fn copy_to_raw_pointer<T: Copy>(ptr: *mut T, index: usize, data: &[T]) {
    unsafe {
        let target_ptr = ptr.add(index);
        for (i, &value) in data.iter().enumerate() {
            *target_ptr.add(i) = value;
        }
    }
}

use core::slice;
use std::collections::HashMap;

pub fn parse_sprite_texture_array(ptr: *mut i32, len: usize) -> HashMap<i32, (i32, i32)> {
    let mut map = HashMap::new();

    // Convert raw pointer to a safe slice
    let data: &[i32] = unsafe { slice::from_raw_parts(ptr, len) };

    // Process chunks of 3 (type, height, width)
    for chunk in data.chunks(3) {
        if let [sprite_type, height, width] = *chunk {
            map.insert(sprite_type, (height, width));
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
    pub pitch: f32,
    pub z: f32,
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
    pub angle: i32,
    pub height: i32,
    pub r#type: i32,
}

#[wasm_bindgen]
#[derive(Debug)]
pub struct TranslationResult {
    pub screen_x: f32,
    pub screen_y_floor: f32,
    pub screen_y_ceiling: f32,
    pub distance: f32,
    pub full_height: f32,
    pub transform_x: f32,
    pub transform_y: f32,
}

#[wasm_bindgen]
#[derive(Serialize)]
pub struct StripePart {
    pub sprite_type: i32,
    pub stripe_left_x: i32,
    pub stripe_right_x: i32,
    pub screen_y_ceiling: i32,
    pub screen_y_floor: i32,
    pub tex_x1: i32,
    pub tex_x2: i32,
    pub alpha: i32,
    pub angle: i32,
}

pub fn is_in_grid(map_x: i32, map_y: i32, map_width: i32, map_data: &Vec<u8>) -> (bool, u8) {
    let map_index = (map_y) * (map_width) + (map_x);

    if map_y >= 0 && map_x >= 0 && map_index < (map_width * map_width) as i32 && map_index >= 0 {
        return (true, map_data[map_index as usize]);
    }
    (false, 0)
}

pub fn is_of_value_in_grid(
    map_x: i32,
    map_y: i32,
    map_width: i32,
    map_data: &Vec<u8>,
    values: &[u8],
) -> (bool, u8) {
    let map_index = map_y * map_width + map_x;

    if map_y >= 0 && map_x >= 0 && map_index >= 0 && map_index < (map_width * map_width) as i32 {
        let value = map_data[map_index as usize];
        return (values.contains(&value), value);
    }
    (false, 0)
}
