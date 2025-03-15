use wasm_bindgen::prelude::*;
use serde_wasm_bindgen::to_value;

#[wasm_bindgen]
pub fn wasm_test() -> String {
    "Hello Moitsa, from wasm!".to_string()
}

#[wasm_bindgen]
pub struct WallDrawInfo {
    pub tex_x: i32,
    pub left: i32,
    pub draw_start_y: i32,
    pub wall_height: i32,
    pub global_alpha: f32,  // Transparency value
    pub perp_wall_dist: f32,
    pub hit:u8,
    
}

#[wasm_bindgen]
pub fn draw_walls_raycast(
    player_x: f32,
    player_y: f32,
    player_dir_x: f32,
    player_dir_y: f32,
    player_plane_x: f32,
    player_plane_y: f32,
    map_data: Vec<u8>,  // 2D array representing the grid map
    map_width: i32,      // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width:i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    plane_y_initial: f32,
    pitch: f32,
    z: f32,
    wall_texture_width: i32,
) -> Vec<WallDrawInfo> {
    let mut result: Vec<WallDrawInfo> = Vec::new();

    for column in 0..width_resolution {
        let camera_x = (2.0 * (column as f32) / (width_resolution as f32)) - 1.0;

        let ray_dir_x = player_dir_x + player_plane_x * camera_x;
        let ray_dir_y = player_dir_y + player_plane_y * camera_x;

        let mut map_x = player_x.floor() as i32;
        let mut map_y = player_y.floor() as i32;

        let delta_dist_x = ray_dir_x.abs().recip();
        let delta_dist_y = ray_dir_y.abs().recip();

        let mut perp_wall_dist = 0.0;
        let mut step_x:i8 = 0;
        let mut step_y:i8 = 0;
        let mut side = 0;

        let mut side_dist_x = 0.0;
        let mut side_dist_y = 0.0;

        if ray_dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (player_x - map_x as f32) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = (map_x as f32 + 1.0 - player_x) * delta_dist_x;
        }

        if ray_dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (player_y - map_y as f32) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = (map_y as f32 + 1.0 - player_y) * delta_dist_y;
        }

        let mut hit:u8 = 0;
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

            let map_index = (map_y * map_width + map_x) ;
            if map_y >= 0 && map_x >= 0 && map_index < map_data.len() as i32 && map_index >= 0 && map_data[map_index as usize] == 1 {
                hit = 1;
            }

            remaining_range -= 1;
        }

        if side == 0 {
            perp_wall_dist = side_dist_x - delta_dist_x;
        } else {
            perp_wall_dist = side_dist_y - delta_dist_y;
        }

        let line_height = width as f32 / 2.0 / plane_y_initial / perp_wall_dist;

        let draw_start_y = -line_height / 2.0 + height as f32 / 2.0 + pitch + z;
        let draw_end_y = line_height / 2.0 + height as f32 / 2.0 + pitch + z;

        let mut wall_x:f32;
        if side == 0 {
            wall_x = player_y + perp_wall_dist * ray_dir_y;
        } else {
            wall_x = player_x + perp_wall_dist * ray_dir_x;
        }

        wall_x -= wall_x.floor();

        let tex_x = (wall_x * wall_texture_width as f32) as i32;
        let tex_x = if side == 0 && ray_dir_x > 0.0 { wall_texture_width - tex_x - 1 } else { tex_x };
        let tex_x = if side == 1 && ray_dir_y < 0.0 { wall_texture_width - tex_x - 1 } else { tex_x };

        // Calculate globalAlpha based on light range and distance
        let mut global_alpha = perp_wall_dist / light_range;
        if global_alpha > 0.8 {
            global_alpha =   0.8; // Ensure minimum visibility
        }
        if side == 1 {
            // give x and y sides different brightness
            global_alpha = global_alpha * 2.0;
          }
        if global_alpha > 0.85 {
            global_alpha =   0.85; // Ensure minimum visibility
        }

        result.push(WallDrawInfo {
            tex_x,
            left: ((column as f32 * width_spacing).ceil() as i32) as i32,
            draw_start_y: draw_start_y as i32,
            wall_height: (draw_end_y - draw_start_y) as i32,
            global_alpha,
            perp_wall_dist,
            hit,
        });
    }

    result
}

use serde::{Serialize, Deserialize};
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
    pub x: f64,
    pub y: f64,
    pub r#type: i32,
}

#[derive(Serialize, Deserialize)]
pub struct RaycastResult {
    pub coords: HashMap<String, Coords>,
    pub sprites: Vec<Sprite>,
}

#[wasm_bindgen]
pub fn raycast_visible_coordinates(
    width_resolution: usize,
    range: i32,
    player_x: f64,
    player_y: f64,
    dir_x: f64,
    dir_y: f64,
    plane_x: f64,
    plane_y: f64,
    map_data: Vec<u8>,  // 2D array representing the grid map
    map_width: i32,      // Needed to index into 1D map
    sprite_data: &[f64], // Flattened array [x1, y1, type1, x2, y2, type2, ...]
) -> JsValue {
    let mut coords: HashMap<String, Coords> = HashMap::new();
    let mut sprites: Vec<Sprite> = Vec::new();

    let mut sprites_map: HashMap<(i32, i32), Vec<Sprite>> = HashMap::new();

    // transform sprites into a hash map with floored coords for easy access
    for i in (0..sprite_data.len()).step_by(3) {
        let sx = sprite_data[i];
        let sy = sprite_data[i + 1];
        let sprite_type = sprite_data[i + 2] as i32;
    
        let key = (sx.floor() as i32, sy.floor() as i32);
    
        sprites_map.entry(key)
            .or_insert_with(Vec::new)
            .push(Sprite { x: sx, y: sy, r#type: sprite_type });
    }

    for column in 0..width_resolution {
        let camera_x = 2.0 * column as f64 / width_resolution as f64 - 1.0;
        let ray_dir_x = dir_x + plane_x * camera_x;
        let ray_dir_y = dir_y + plane_y * camera_x;

        let mut map_x = player_x.floor() as i32;
        let mut map_y = player_y.floor() as i32;

        let delta_dist_x = (1.0 / ray_dir_x).abs();
        let delta_dist_y = (1.0 / ray_dir_y).abs();

        let mut side_dist_x;
        let mut side_dist_y;
        let step_x;
        let step_y;

        if ray_dir_x < 0.0 {
            step_x = -1;
            side_dist_x = (player_x - map_x as f64) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x = ((map_x + 1) as f64 - player_x) * delta_dist_x;
        }

        if ray_dir_y < 0.0 {
            step_y = -1;
            side_dist_y = (player_y - map_y as f64) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y = ((map_y + 1) as f64 - player_y) * delta_dist_y;
        }

        let mut hit = false;
        let mut current_range = range;

        while !hit && current_range >= 0 {
            let index = (map_y * map_width + map_x);
            
            let map_value:u8;
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
                        sprites.push(Sprite { x: sprite.x, y: sprite.y, r#type: sprite.r#type });
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

#[derive(Serialize, Deserialize)]
pub struct CeilingFloorResult {
    pub black_pixels: Vec<u8>,
    pub texture_pixels:Vec<u8>
}

use rayon::prelude::*;


#[wasm_bindgen]
pub fn draw_ceiling_floor_raycast(
    ceiling_width_resolution: usize,
    ceiling_height_resolution: usize,
    light_range: f32,
    map_light: f32,
    player_x: f32,
    player_y: f32,
    player_dir_x: f32,
    player_dir_y: f32,
    player_plane_x: f32,
    player_plane_y: f32,
    player_pitch: f32,
    player_z: f32,
    floor_texture: &[u8],
    floor_texture_width: usize,
    floor_texture_height: usize,
    ceiling_texture: &[u8],
    ceiling_texture_width: usize,
    ceiling_texture_height: usize,
    map_data: &[u8],
    map_width: usize,
    map_height: usize,
    base_height: f32
) -> JsValue {
    let mut ceiling_floor_img = vec![0; ceiling_width_resolution * ceiling_height_resolution * 4];
    let mut floor_img_black_pixels = vec![0; ceiling_width_resolution * ceiling_height_resolution * 4];

    let ray_dir_x0 = player_dir_x - player_plane_x;
    let ray_dir_y0 = player_dir_y - player_plane_y;
    let ray_dir_x1 = player_dir_x + player_plane_x;
    let ray_dir_y1 = player_dir_y + player_plane_y;
    let ray_dir_x_dist = ray_dir_x1 - ray_dir_x0;
    let ray_dir_y_dist = ray_dir_y1 - ray_dir_y0;

    let half_height = ceiling_height_resolution as f32 / 2.0;
    let scale = ceiling_height_resolution as f32 / base_height;
    let scaled_pitch = player_pitch * scale;
    let scaled_z = player_z * scale;

    for y in 0..ceiling_height_resolution {
        let is_floor = (y as f32) > half_height + scaled_pitch;
        
        let p = if is_floor {
            y as f32 - half_height - scaled_pitch
        } else {
            half_height - y as f32 + scaled_pitch
        };
        let cam_z = if is_floor { half_height + scaled_z } else { half_height - scaled_z };
        let row_distance = cam_z / p;
        let mut alpha = (row_distance + 0.0) / light_range - map_light;
        alpha = alpha.min(0.8);

        let floor_step_x = (row_distance * ray_dir_x_dist) / ceiling_width_resolution as f32;
        let floor_step_y = (row_distance * ray_dir_y_dist) / ceiling_width_resolution as f32;
        let mut floor_x = player_x + row_distance * ray_dir_x0;
        let mut floor_y = player_y + row_distance * ray_dir_y0;
        let row_alpha = ((1.0 - alpha) * 255.0) as u8;

        let (texture, texture_width, texture_height) = if is_floor {
            (floor_texture, floor_texture_width, floor_texture_height)
        } else {
            (ceiling_texture, ceiling_texture_width, ceiling_texture_height)
        };

        for x in 0..ceiling_width_resolution {
            floor_x += floor_step_x;
            floor_y += floor_step_y;

            let map_idx = (floor_x as usize) + (floor_y as usize) * map_width;
            if map_data.get(map_idx) != Some(&2) {
                continue;
            }

            let cell_x = floor_x.fract();
            let cell_y = floor_y.fract();

            let tx = (texture_width as f32 * cell_x) as usize;
            let ty = (texture_height as f32 * cell_y) as usize;
            let tex_idx = (ty * texture_width + tx) * 4;

            if let Some(slice) = texture.get(tex_idx..tex_idx + 3) {
                let pixel_idx = (y * ceiling_width_resolution + x) * 4;
                ceiling_floor_img[pixel_idx..pixel_idx + 3].copy_from_slice(slice);
                ceiling_floor_img[pixel_idx + 3] = row_alpha;
                floor_img_black_pixels[pixel_idx..pixel_idx + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }

    let result = CeilingFloorResult { black_pixels:floor_img_black_pixels, texture_pixels:ceiling_floor_img };
    to_value(&result).unwrap() // Convert Rust struct to JsValue and return it
}
