use helpers::{
    copy_to_raw_pointer, parse_sprite_texture_array, Coords, Position, RaycastResult, Sprite,
    StripePart, TranslationResult,
};
use js_sys::Math::atan2;
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
mod helpers;
use std::collections::HashMap;

#[wasm_bindgen]
pub fn draw_walls_raycast(
    columns_array: *mut i32,
    zbuffer_array: *mut f32,
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

        let mut wall_x: f32;
        if side == 0 {
            wall_x = position.y + perp_wall_dist * ray_dir_y;
        } else {
            wall_x = position.x + perp_wall_dist * ray_dir_x;
        }

        perp_wall_dist = perp_wall_dist * position.plane_y_initial;
        let line_height = width as f32 / 2.0 / perp_wall_dist;

        let draw_start_y =
            -line_height / 2.0 + height as f32 / 2.0 + position.pitch + position.z / perp_wall_dist;
        let draw_end_y =
            line_height / 2.0 + height as f32 / 2.0 + position.pitch + position.z / perp_wall_dist;

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
                hit as i32,
            ],
        );
        copy_to_raw_pointer(zbuffer_array, column as usize, &[perp_wall_dist]);
    }
}

#[wasm_bindgen]
pub fn raycast_visible_coordinates(
    position: JsValue,
    width_resolution: usize,
    range: i32,
    map_data: Vec<u8>, // 2D array representing the grid map // TODO: to shared memory?
    map_width: i32,    // Needed to index into 1D map
    sprite_array: *mut f32, // Flattened array [x1, y1, angle1, type1, x2, y2, angle2, type2, ...]
    sprites_count: usize,
) -> JsValue {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let sprite_data = unsafe { std::slice::from_raw_parts(sprite_array, sprites_count * 4) };

    // to make sure we dedupe the sprites
    let mut coords: HashMap<String, Coords> = HashMap::new();
    let mut sprites: Vec<Sprite> = Vec::new();

    let mut sprites_map: HashMap<(i32, i32), Vec<Sprite>> = HashMap::new();

    // transform sprites into a hash map with floored coords for easy access
    for i in (0..sprites_count * 4).step_by(4) {
        let sx = sprite_data[i];
        let sy = sprite_data[i + 1];
        let sprite_angle = sprite_data[i + 2] as i32;
        let sprite_type = sprite_data[i + 3] as i32;

        let key = (sx.floor() as i32, sy.floor() as i32);

        sprites_map
            .entry(key)
            .or_insert_with(Vec::new)
            .push(Sprite {
                x: sx,
                y: sy,
                angle: sprite_angle,
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
                            angle: sprite.angle,
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

    let result = RaycastResult { sprites };
    to_value(&result).unwrap() // Convert Rust struct to JsValue and return it // TODO: just set directly and don't pass around?
}

#[wasm_bindgen]
pub fn draw_ceiling_floor_raycast(
    position: JsValue,
    ceiling_floor_img: *mut u8,
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
        // blank out the whole image buffer
        std::ptr::write_bytes(
            ceiling_floor_img,
            0,
            ceiling_width_resolution * ceiling_height_resolution * 4,
        );
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
            let is_floor = (y as f32) > half_height + scaled_pitch;

            let p = if is_floor {
                y as f32 - half_height - scaled_pitch
            } else {
                half_height - y as f32 + scaled_pitch
            };
            let cam_z = if is_floor {
                half_height + scaled_z
            } else {
                half_height - scaled_z
            };

            let row_distance = cam_z
                / p
                / (ceiling_width_resolution as f32 / ceiling_height_resolution as f32)
                / 2.0
                / position.plane_y_initial;
            let mut alpha = (row_distance + 0.0) / light_range - map_light;
            alpha = alpha.min(0.8);

            let floor_step_x = (row_distance * ray_dir_x_dist) / ceiling_width_resolution as f32;
            let floor_step_y = (row_distance * ray_dir_y_dist) / ceiling_width_resolution as f32;
            let mut floor_x = position.x + row_distance * ray_dir_x0;
            let mut floor_y = position.y + row_distance * ray_dir_y0;

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

                if map_data.get(map_idx) != Some(&2) {
                    continue;
                }

                let pixel_idx = (y * ceiling_width_resolution + x) * 4;

                let cell_x = floor_x.fract();
                let cell_y = floor_y.fract();

                let tx = (texture_width as f32 * cell_x) as usize;
                let ty = (texture_height as f32 * cell_y) as usize;
                let tex_idx = (ty * texture_width + tx) * 4;

                let texture_ptr = texture.offset(tex_idx as isize); // Assuming tex_idx is within bounds

                let darkening_factor = 1.0 - alpha; // Adjust for the desired darkness
                let r = (*texture_ptr as f32 * darkening_factor) as u8;
                let g = (*texture_ptr.add(1) as f32 * darkening_factor) as u8;
                let b = (*texture_ptr.add(2) as f32 * darkening_factor) as u8;

                copy_to_raw_pointer(ceiling_floor_img, pixel_idx, &[r, g, b, 255]);
            }
        }
    }
}

pub fn translate_coordinate_to_camera(
    position: Position,
    point_x: f32,
    point_y: f32,
    height_multiplier: f32,
    width: i32,
    height: i32,
) -> TranslationResult {
    // translate x, y position to relative to camera
    let sprite_x = point_x - position.x;
    let sprite_y = point_y - position.y;

    // inverse camera matrix calculation
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();
    let transform_x = inv_det * (position.dir_y * sprite_x - position.dir_x * sprite_y)
        / position.plane_y_initial;
    let transform_y = (inv_det * (-position.plane_y * sprite_x + position.plane_x * sprite_y))
        / position.plane_y_initial;

    let screen_x = (width as f32 / 2.0) * (1.0 + (transform_x / transform_y));

    // to control the pitch/jump
    let v_move_screen = position.pitch + position.z / transform_y;

    // divide by focal length (length of the plane vector)
    let y_height_before_adjustment = (width as f32 / 2.0 / (transform_y)).abs();
    let y_height = y_height_before_adjustment * height_multiplier;
    let height_distance = y_height_before_adjustment - y_height;
    let screen_ceiling_y = height as f32 / 2.0 - y_height / 2.0;
    let screen_floor_y = height as f32 / 2.0 + y_height / 2.0;
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

#[wasm_bindgen]
pub fn draw_sprites_wasm(
    position_js: JsValue,
    width: i32,
    height: i32,
    width_spacing: i32,
    sprites_array: *mut f32,
    sprites_count: usize,
    zbuffer_array: *mut f32,
    zbuffer_length: usize,
    sprites_texture_array: *mut i32,
    sprites_texture_array_length: usize,
    light_range: f32,
    map_light: f32,
) -> JsValue {
    let position: Position = serde_wasm_bindgen::from_value(position_js).unwrap();
    let zbuffer = unsafe { std::slice::from_raw_parts(zbuffer_array, zbuffer_length) };
    let sprite_data = unsafe { std::slice::from_raw_parts(sprites_array, sprites_count * 4) };
    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    let mut sprites = Vec::new();
    for i in (0..sprites_count * 4).step_by(4) {
        sprites.push(Sprite {
            x: sprite_data[i],
            y: sprite_data[i + 1],
            angle: sprite_data[i + 2] as i32,
            r#type: sprite_data[i + 3] as i32,
        });
    }

    let mut stripe_parts = Vec::new();

    sprites.sort_by(|a, b| {
        let da = (position.x - a.x).powi(2) + (position.y - a.y).powi(2);
        let db = (position.x - b.x).powi(2) + (position.y - b.y).powi(2);
        db.partial_cmp(&da).unwrap()
    });

    for sprite in sprites.iter() {
        // TODO: this is causing the first one to disappear??
        let (texture_height, texture_width, texture_multiplier) = texture_array
            .get(&sprite.r#type)
            .copied()
            .unwrap_or((100, 100, 100));

        let aspect_ratio = texture_width as f32 / texture_height as f32;

        let projection = translate_coordinate_to_camera(
            position,
            sprite.x,
            sprite.y,
            texture_multiplier as f32 / 100.0, // Placeholder texture height - the multiplier not actual height
            width,
            height,
        );

        let dx = position.x - sprite.x;
        let dy = position.y - sprite.y;
        let angle = atan2(dx as f64, dy as f64);
        // will return from -180 to 180
        let angle_i = (((angle).to_degrees() as i32) + 180 + sprite.angle) % 360;

        let alpha = projection.distance / light_range - map_light;
        // ensure sprites are always at least a little bit visible - alpha 1 is all black
        let alpha_i = (100.0 - alpha * 100.0).floor().clamp(20.0, 100.0) as i32;

        let sprite_width = (projection.full_height * aspect_ratio as f32).abs() as i32;

        let draw_start_x = (-sprite_width as f32 / 2.0 + projection.screen_x).max(0.0) as i32;
        let draw_end_x =
            (sprite_width as f32 / 2.0 + projection.screen_x).min(width as f32 - 1.0) as i32;

        let mut stripe_parts_temp = Vec::new();
        for stripe in (draw_start_x..draw_end_x).step_by(width_spacing as usize) {
            if projection.distance > 0.0 && stripe >= 0 && stripe < width {
                let z_index = (stripe / width_spacing) as usize;
                if projection.distance < zbuffer[z_index] {
                    if stripe_parts_temp.len() % 2 == 0 {
                        stripe_parts_temp.push(stripe);
                    }
                    if stripe + width_spacing >= draw_end_x && stripe_parts_temp.len() % 2 == 1 {
                        stripe_parts_temp.push(stripe);
                    }
                } else if stripe_parts_temp.len() % 2 == 1 {
                    stripe_parts_temp.push(stripe);
                }
            }
        }

        for pair in stripe_parts_temp.chunks_exact(2) {
            let sprite_width_f64 = sprite_width as f64;
            let screen_x_f64 = projection.screen_x as f64;

            let tex_x1 = (((pair[0] as f64 - (-sprite_width_f64 / 2.0 + screen_x_f64))
                * texture_width as f64)
                / sprite_width_f64) as i32;
            let tex_x2 = (((pair[1] as f64 - (-sprite_width_f64 / 2.0 + screen_x_f64))
                * texture_width as f64)
                / sprite_width_f64) as i32;

            stripe_parts.push(StripePart {
                sprite_type: sprite.r#type,
                stripe_left_x: pair[0],
                stripe_right_x: pair[1],
                screen_y_ceiling: projection.screen_y_ceiling as i32,
                screen_y_floor: projection.screen_y_floor as i32,
                tex_x1,
                tex_x2,
                alpha: alpha_i,
                angle: angle_i,
            });
        }
    }

    serde_wasm_bindgen::to_value(&stripe_parts).unwrap()
}
