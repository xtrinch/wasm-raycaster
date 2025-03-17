use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;

#[derive(Serialize, Deserialize)]
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

#[wasm_bindgen]
pub fn draw_walls_raycast(
    columns_array: *mut i32,
    position: JsValue,
    map_data: Vec<u8>, // 2D array representing the grid map
    map_width: i32,    // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
) -> () {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();

    for column in 0..width_resolution {
        let camera_x = (2.0 * (column as f32) / (width_resolution as f32)) - 1.0;

        let ray_dir_x = position.dir_x + position.plane_x * camera_x;
        let ray_dir_y = position.dir_y + position.plane_y * camera_x;

        let mut map_x = position.x.floor() as i32;
        let mut map_y = position.y.floor() as i32;

        let delta_dist_x = ray_dir_x.abs().recip();
        let delta_dist_y = ray_dir_y.abs().recip();

        let mut perp_wall_dist = 0.0;
        let mut step_x: i8 = 0;
        let mut step_y: i8 = 0;
        let mut side = 0;

        let mut side_dist_x = 0.0;
        let mut side_dist_y = 0.0;

        if ray_dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (position.x - map_x as f32) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = (map_x as f32 + 1.0 - position.x) * delta_dist_x;
        }

        if ray_dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (position.y - map_y as f32) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = (map_y as f32 + 1.0 - position.y) * delta_dist_y;
        }

        let mut hit: u8 = 0;
        let mut remaining_range = range;
        while hit == 0 && remaining_range >= 0 {
            if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x as i32;
                side = 0;
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y as i32;
                side = 1;
            }

            let map_index = (map_y * map_width + map_x);
            if map_y >= 0
                && map_x >= 0
                && map_index < map_data.len() as i32
                && map_index >= 0
                && map_data[map_index as usize] == 1
            {
                hit = 1;
            }

            remaining_range -= 1;
        }

        if side == 0 {
            perp_wall_dist = side_dist_x - delta_dist_x;
        } else {
            perp_wall_dist = side_dist_y - delta_dist_y;
        }

        let line_height = width as f32 / 2.0 / position.plane_y_initial / perp_wall_dist;

        let draw_start_y = -line_height / 2.0 + height as f32 / 2.0 + position.pitch + position.z;
        let draw_end_y = line_height / 2.0 + height as f32 / 2.0 + position.pitch + position.z;

        let mut wall_x: f32;
        if side == 0 {
            wall_x = position.y + perp_wall_dist * ray_dir_y;
        } else {
            wall_x = position.x + perp_wall_dist * ray_dir_x;
        }

        wall_x -= wall_x.floor();

        let tex_x = (wall_x * wall_texture_width as f32) as i32;
        let tex_x = if side == 0 && ray_dir_x > 0.0 {
            wall_texture_width - tex_x - 1
        } else {
            tex_x
        };
        let tex_x = if side == 1 && ray_dir_y < 0.0 {
            wall_texture_width - tex_x - 1
        } else {
            tex_x
        };

        // Calculate globalAlpha based on light range and distance
        let mut global_alpha = perp_wall_dist / light_range;
        if global_alpha > 0.8 {
            global_alpha = 0.8; // Ensure minimum visibility
        }
        if side == 1 {
            // give x and y sides different brightness
            global_alpha = global_alpha * 2.0;
        }
        if global_alpha > 0.85 {
            global_alpha = 0.85; // Ensure minimum visibility
        }

        let left = ((column as f32 * width_spacing).ceil() as i32) as i32;
        let wall_height = (draw_end_y - draw_start_y) as i32;
        let array_idx = 7 * column as usize;
        copy_to_raw_pointer(
            columns_array,
            array_idx,
            &[
                tex_x,
                left,
                draw_start_y as i32,
                wall_height,
                (global_alpha * 100.0) as i32,
                (perp_wall_dist * 100.0) as i32,
                hit as i32,
            ],
        );
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Data structures
#[derive(Serialize, Deserialize)]
#[wasm_bindgen]
pub struct Coords {
    pub x: i32,
    pub y: i32,
    pub has_ceiling_floor: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Sprite {
    pub x: f32,
    pub y: f32,
    pub r#type: i32,
}

#[derive(Serialize, Deserialize)]
pub struct RaycastResult {
    pub coords: HashMap<String, Coords>,
    pub sprites: Vec<Sprite>,
}

#[wasm_bindgen]
pub fn raycast_visible_coordinates(
    position: JsValue,
    width_resolution: usize,
    range: i32,
    map_data: Vec<u8>,   // 2D array representing the grid map
    map_width: i32,      // Needed to index into 1D map
    sprite_data: &[f32], // Flattened array [x1, y1, type1, x2, y2, type2, ...]
) -> JsValue {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();

    let mut coords: HashMap<String, Coords> = HashMap::new();
    let mut sprites: Vec<Sprite> = Vec::new();

    let mut sprites_map: HashMap<(i32, i32), Vec<Sprite>> = HashMap::new();

    // transform sprites into a hash map with floored coords for easy access
    for i in (0..sprite_data.len()).step_by(3) {
        let sx = sprite_data[i];
        let sy = sprite_data[i + 1];
        let sprite_type = sprite_data[i + 2] as i32;

        let key = (sx.floor() as i32, sy.floor() as i32);

        sprites_map
            .entry(key)
            .or_insert_with(Vec::new)
            .push(Sprite {
                x: sx,
                y: sy,
                r#type: sprite_type,
            });
    }

    for column in 0..width_resolution {
        let camera_x = 2.0 * column as f32 / width_resolution as f32 - 1.0;
        let ray_dir_x = position.dir_x + position.plane_x * camera_x;
        let ray_dir_y = position.dir_y + position.plane_y * camera_x;

        let mut map_x = position.x.floor() as i32;
        let mut map_y = position.y.floor() as i32;

        let delta_dist_x = (1.0 / ray_dir_x).abs();
        let delta_dist_y = (1.0 / ray_dir_y).abs();

        let mut side_dist_x;
        let mut side_dist_y;
        let step_x;
        let step_y;

        if ray_dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (position.x - map_x as f32) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = ((map_x + 1) as f32 - position.x) * delta_dist_x;
        }

        if ray_dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (position.y - map_y as f32) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = ((map_y + 1) as f32 - position.y) * delta_dist_y;
        }

        let mut hit = false;
        let mut current_range = range;

        while !hit && current_range >= 0 {
            let index = (map_y * map_width + map_x);

            let map_value: u8;
            if map_y < 0 || map_x < 0 {
                map_value = 0
            } else {
                map_value = map_data.get(index as usize).copied().unwrap_or(0);
            }

            if map_value == 1 {
                hit = true;
            }

            let coord_key = format!("{}-{}", map_x, map_y);
            if !coords.contains_key(&coord_key) {
                let has_ceiling_floor = map_value == 2;
                coords.insert(
                    coord_key.clone(),
                    Coords {
                        x: map_x,
                        y: map_y,
                        has_ceiling_floor,
                    },
                );

                if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
                    for sprite in sprite_list {
                        sprites.push(Sprite {
                            x: sprite.x,
                            y: sprite.y,
                            r#type: sprite.r#type,
                        });
                    }
                }
            }

            if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x;
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y;
            }
            current_range -= 1;
        }
    }

    let result = RaycastResult { coords, sprites };
    to_value(&result).unwrap() // Convert Rust struct to JsValue and return it
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
}

fn copy_to_raw_pointer<T: Copy>(ptr: *mut T, index: usize, data: &[T]) {
    unsafe {
        let target_ptr = ptr.add(index);
        for (i, &value) in data.iter().enumerate() {
            *target_ptr.add(i) = value;
        }
    }
}

#[wasm_bindgen]
pub fn draw_ceiling_floor_raycast(
    position: JsValue,
    ceiling_floor_img: *mut u8,
    floor_img_black_pixels: *mut u8,
    floor_texture: *mut u8,
    ceiling_texture: *mut u8,
    ceiling_width_resolution: usize,
    ceiling_height_resolution: usize,
    light_range: f32,
    map_light: f32,
    floor_texture_width: usize,
    floor_texture_height: usize,
    ceiling_texture_width: usize,
    ceiling_texture_height: usize,
    map_data: &[u8],
    map_width: usize,
    base_height: f32,
) -> () {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();

    unsafe {
        let ray_dir_x0 = position.dir_x - position.plane_x;
        let ray_dir_y0 = position.dir_y - position.plane_y;
        let ray_dir_x1 = position.dir_x + position.plane_x;
        let ray_dir_y1 = position.dir_y + position.plane_y;
        let ray_dir_x_dist = ray_dir_x1 - ray_dir_x0;
        let ray_dir_y_dist = ray_dir_y1 - ray_dir_y0;

        let half_height = ceiling_height_resolution as f32 / 2.0;
        let scale = ceiling_height_resolution as f32 / base_height;
        let scaled_pitch = position.pitch * scale;
        let scaled_z = position.z * scale;

        for y in 0..ceiling_height_resolution {
            let is_floor = (y as f32) > half_height + scaled_pitch + scaled_z;

            let p = if is_floor {
                y as f32 - half_height - scaled_pitch - scaled_z
            } else {
                half_height - y as f32 + scaled_pitch + scaled_z
            };
            let cam_z = half_height;
            let row_distance = cam_z
                / p
                / (ceiling_width_resolution as f32 / ceiling_height_resolution as f32)
                / 2.0;
            let mut alpha = (row_distance + 0.0) / light_range - map_light;
            alpha = alpha.min(0.8);

            let floor_step_x = (row_distance * ray_dir_x_dist) / ceiling_width_resolution as f32;
            let floor_step_y = (row_distance * ray_dir_y_dist) / ceiling_width_resolution as f32;
            let mut floor_x = position.x + row_distance * ray_dir_x0;
            let mut floor_y = position.y + row_distance * ray_dir_y0;
            let row_alpha = ((1.0 - alpha) * 255.0) as u8;

            let (texture, texture_width, texture_height) = if is_floor {
                (floor_texture, floor_texture_width, floor_texture_height)
            } else {
                (
                    ceiling_texture,
                    ceiling_texture_width,
                    ceiling_texture_height,
                )
            };

            for x in 0..ceiling_width_resolution {
                floor_x += floor_step_x;
                floor_y += floor_step_y;

                let map_idx = (floor_x as usize) + (floor_y as usize) * map_width;
                let pixel_idx = (y * ceiling_width_resolution + x) * 4;

                if map_data.get(map_idx) != Some(&2) {
                    copy_to_raw_pointer(ceiling_floor_img, pixel_idx, &[0, 0, 0, 0]);
                    copy_to_raw_pointer(floor_img_black_pixels, pixel_idx, &[0, 0, 0, 0]);
                    continue;
                }

                let cell_x = floor_x.fract();
                let cell_y = floor_y.fract();

                let tx = (texture_width as f32 * cell_x) as usize;
                let ty = (texture_height as f32 * cell_y) as usize;
                let tex_idx = (ty * texture_width + tx) * 4;

                let texture_ptr = texture.offset(tex_idx as isize); // Assuming tex_idx is within bounds
                let r = *texture_ptr;
                let g = *texture_ptr.add(1);
                let b = *texture_ptr.add(2);

                copy_to_raw_pointer(ceiling_floor_img, pixel_idx, &[r, g, b, row_alpha]);
                copy_to_raw_pointer(floor_img_black_pixels, pixel_idx, &[0, 0, 0, 255]);
            }
        }
    }
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
pub fn translate_coordinate_to_camera(
    position: JsValue,
    point_x: f32,
    point_y: f32,
    height_multiplier: f32,
    width: f32,
    height: f32,
) -> TranslationResult {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();

    // translate x, y position to relative to camera
    let sprite_x = point_x - position.x;
    let sprite_y = point_y - position.y;

    // inverse camera matrix calculation
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();
    let transform_x = inv_det * (position.dir_y * sprite_x - position.dir_x * sprite_y);
    let transform_y = inv_det * (-position.plane_y * sprite_x + position.plane_x * sprite_y);

    let screen_x = (width / 2.0) * (1.0 + (transform_x / transform_y));

    // to control the pitch/jump
    let v_move_screen = position.pitch + position.z;

    // divide by focal length (length of the plane vector)
    let y_height_before_adjustment = (width / 2.0 / position.plane_y_initial / transform_y).abs();
    let y_height = y_height_before_adjustment * height_multiplier;
    let height_distance = y_height_before_adjustment - y_height;
    let screen_ceiling_y = height / 2.0 - y_height / 2.0;
    let screen_floor_y = height / 2.0 + y_height / 2.0;
    let sprite_floor_screen_y = screen_floor_y + v_move_screen + height_distance / 2.0;
    let sprite_ceiling_screen_y = screen_ceiling_y + v_move_screen + height_distance / 2.0;
    let full_height = sprite_ceiling_screen_y - sprite_floor_screen_y;

    TranslationResult {
        screen_x,
        screen_y_floor: sprite_floor_screen_y,
        screen_y_ceiling: sprite_ceiling_screen_y,
        distance: transform_y,
        full_height,
        transform_x,
        transform_y,
    }
}
