use helpers::{
    copy_to_raw_pointer, is_in_grid, is_of_value_in_grid, parse_sprite_texture_array, Coords,
    Position, Sprite, StripePart, TranslationResult,
};
use js_sys::Math::atan2;
use wasm_bindgen::prelude::*;
mod helpers;
mod line_intersection;
use geo::Line;
use line_intersection::LineInterval;
use std::collections::HashMap;
use web_sys::console;

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
        // x-coordinate in camera space
        let camera_x = (2.0 * (column as f32) / (width_resolution as f32)) - 1.0;

        let ray_dir_x = position.dir_x + position.plane_x * camera_x;
        let ray_dir_y = position.dir_y + position.plane_y * camera_x;

        // which box of the map we're in
        let mut map_x = position.x.floor() as i32;
        let mut map_y = position.y.floor() as i32;

        // length of ray from one x or y-side to next x or y-side
        let delta_dist_x = ray_dir_x.abs().recip();
        let delta_dist_y = ray_dir_y.abs().recip();

        let mut perp_wall_dist = 0.0;

        // what direction to step in x or y-direction (either +1 or -1)
        let mut step_x: i8 = 0;
        let mut step_y: i8 = 0;
        let mut side = 0;

        let mut side_dist_x: f32 = 0.0;
        let mut side_dist_y: f32 = 0.0;

        // initial side dists;
        // starting from the player, we find the nearest horizontal (stepX) and vertical (stepY) gridlines
        if ray_dir_x < 0.0 {
            step_x = -1;
            side_dist_x += (position.x - map_x as f32) * delta_dist_x;
        } else {
            step_x = 1;
            side_dist_x += (map_x as f32 + 1.0 - position.x) * delta_dist_x;
        }

        if ray_dir_y < 0.0 {
            step_y = -1;
            side_dist_y += (position.y - map_y as f32) * delta_dist_y;
        } else {
            step_y = 1;
            side_dist_y += (map_y as f32 + 1.0 - position.y) * delta_dist_y;
        }

        let mut hit: u8 = 0;
        let mut hit_type: i8 = 1;
        let mut remaining_range = range;

        while hit == 0 && remaining_range >= 0 {
            // jump to next map square, either in x-direction, or in y-direction
            let mut jump_x: bool = false;
            let mut jump_y: bool = false;
            if side_dist_x < side_dist_y {
                side_dist_x += delta_dist_x;
                map_x += step_x as i32;
                side = 0;
                jump_x = true;
            } else {
                side_dist_y += delta_dist_y;
                map_y += step_y as i32;
                side = 1;
                jump_y = true
            }

            if jump_x {
                let new_map_x = map_x - step_x as i32;
                if let (true, value) = is_of_value_in_grid(
                    new_map_x,
                    map_y,
                    map_width,
                    &map_data,
                    &[4, 5, 8, 9, 12, 13],
                ) {
                    hit_type = value as i8;
                    if ray_dir_x < 0.0 {
                        // west wall hit
                        if value == 4 || value == 8 || value == 12 {
                            hit = 1;
                        }
                    } else if ray_dir_x > 0.0 {
                        // east wall hit
                        if value == 5 || value == 9 || value == 13 {
                            hit = 1;
                        }
                    }
                }
            }

            if (jump_y) {
                let new_map_y = map_y - step_y as i32;
                if let (true, value) = is_of_value_in_grid(
                    map_x,
                    new_map_y,
                    map_width,
                    &map_data,
                    &[6, 7, 10, 11, 14, 15],
                ) {
                    hit_type = value as i8;
                    if ray_dir_y < 0.0 {
                        // north wall hit
                        if value == 6 || value == 10 || value == 14 {
                            hit = 1;
                        }
                    } else if ray_dir_y > 0.0 {
                        // south wall hit
                        if value == 7 || value == 11 || value == 15 {
                            hit = 1;
                        }
                    }
                }
            }

            if let (true, value) = is_of_value_in_grid(
                map_x,
                map_y,
                map_width,
                &map_data,
                &[1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            ) {
                match value {
                    16 => {
                        // from east or west side
                        if jump_x {
                            let offset = 0.5;
                            let mut distance_offset = 0.0;
                            let mut map_x_adder = 0.0;

                            if ray_dir_x < 0.0 {
                                // from east side

                                distance_offset = offset;
                                map_x_adder = 1.0; // + 1 because it's an east door
                            } else if ray_dir_x > 0.0 {
                                // from west side

                                distance_offset = 1.0 - offset;
                                map_x_adder = 0.0;
                            }

                            let perp_wall_disty = side_dist_x - delta_dist_x;
                            let wall_y = position.y + perp_wall_disty * ray_dir_y;

                            // find the intersection of a line segment and an infinite line
                            let new_map_x = map_x as f32 + offset;

                            // the segment of line at the offset of the wall
                            let segment = LineInterval::line_segment(Line {
                                start: (new_map_x as f32, map_y as f32).into(),
                                end: (new_map_x as f32, map_y as f32 + 1.0 as f32).into(),
                            });

                            // ray between player position and point on the EDGE of the wall
                            let line = LineInterval::ray(Line {
                                start: (position.x, position.y).into(),
                                end: (map_x as f32 + map_x_adder, wall_y as f32).into(),
                            });

                            let js: JsValue = vec![map_x as f32, wall_y as f32].into();
                            if ray_dir_x > 0.0 {
                                // console::log_2(&"Znj?".into(), &js);
                            }

                            let intersection = segment.relate(&line).unique_intersection();
                            if let Some(_) = intersection {
                                hit = 1;
                                // move it back for the amount it should move back
                                perp_wall_dist += delta_dist_x * (1.0 - (distance_offset));
                            }
                        }
                    }
                    _ => {}
                }

                match value {
                    1 => {
                        hit = 1;
                    }
                    4 | 8 | 12 => {
                        if jump_x && ray_dir_x > 0.0 {
                            // west wall hit
                            hit = 1;
                        }
                    }
                    5 | 9 | 13 => {
                        if jump_x && ray_dir_x < 0.0 {
                            hit = 1;
                        }
                    }
                    6 | 10 | 14 => {
                        if jump_y && ray_dir_y > 0.0 {
                            // north wall hit
                            hit = 1;
                        }
                    }
                    7 | 11 | 15 => {
                        if jump_y && ray_dir_y < 0.0 {
                            // south wall hit
                            hit = 1;
                        }
                    }
                    _ => {}
                }
                hit_type = value as i8;
            }

            remaining_range -= 1;
        }

        // Calculate distance of perpendicular ray (Euclidean distance would give fisheye effect!)
        if side == 0 {
            perp_wall_dist += side_dist_x - delta_dist_x;
        } else {
            perp_wall_dist += side_dist_y - delta_dist_y;
        }

        let mut wall_x: f32; // where exactly the wall was hit; note that even if it's called wallX, it's actually an y-coordinate of the wall if side==0, but it's always the x-coordinate of the texture.
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
        let array_idx = 8 * column as usize;
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
                hit_type as i32,
            ],
        );
        copy_to_raw_pointer(zbuffer_array, column as usize, &[perp_wall_dist]);
    }
}

pub fn raycast_visible_coordinates(
    position: JsValue,
    width_resolution: usize,
    range: i32,
    map_array: *mut u8,
    map_width: i32,
    all_sprites_array: *mut f32,
    sprites_count: usize,
    found_sprites_array: *mut f32,
) -> usize {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let all_sprites_data =
        unsafe { std::slice::from_raw_parts(all_sprites_array, sprites_count * 5) };
    let map_data =
        unsafe { std::slice::from_raw_parts(map_array, (map_width * map_width) as usize) };
    let found_sprites =
        unsafe { std::slice::from_raw_parts_mut(found_sprites_array, sprites_count * 5) };

    let mut coords: HashMap<String, Coords> = HashMap::new();
    let mut sprites_map: HashMap<(i32, i32), Vec<[f32; 5]>> = HashMap::new();
    let mut found_sprites_count = 0;

    // map them by x & y coordinate for easy access
    for i in (0..sprites_count * 5).step_by(5) {
        let sprite_data: [_; 5] = all_sprites_data[i..i + 5].try_into().unwrap();

        let key = (sprite_data[0].floor() as i32, sprite_data[1].floor() as i32);

        sprites_map
            .entry(key)
            .or_insert_with(Vec::new)
            .push(sprite_data);
    }

    for column in 0..width_resolution {
        let camera_x = 2.0 * column as f32 / width_resolution as f32 - 1.0;
        let ray_dir_x = position.dir_x + position.plane_x * camera_x;
        let ray_dir_y = position.dir_y + position.plane_y * camera_x;

        let mut map_x = position.x.floor() as i32;
        let mut map_y = position.y.floor() as i32;
        let delta_dist_x = (1.0 / ray_dir_x).abs();
        let delta_dist_y = (1.0 / ray_dir_y).abs();

        let (mut side_dist_x, mut step_x) = if ray_dir_x < 0.0 {
            ((position.x - map_x as f32) * delta_dist_x, -1)
        } else {
            (((map_x + 1) as f32 - position.x) * delta_dist_x, 1)
        };
        let (mut side_dist_y, mut step_y) = if ray_dir_y < 0.0 {
            ((position.y - map_y as f32) * delta_dist_y, -1)
        } else {
            (((map_y + 1) as f32 - position.y) * delta_dist_y, 1)
        };

        let mut hit = false;
        let mut current_range = range;

        while !hit && current_range >= 0 {
            let index = (map_y * map_width + map_x) as usize;
            let map_value = if map_y < 0 || map_x < 0 {
                0
            } else {
                map_data.get(index).copied().unwrap_or(0)
            };

            if map_value == 1 {
                hit = true;
            }

            let coord_key = format!("{}-{}", map_x, map_y);
            if !coords.contains_key(&coord_key) {
                coords.insert(coord_key.clone(), Coords { x: map_x, y: map_y });

                if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
                    for &sprite in sprite_list {
                        found_sprites[found_sprites_count * 5..found_sprites_count * 5 + 5]
                            .copy_from_slice(&sprite);
                        found_sprites_count += 1;
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

    found_sprites_count
}

#[wasm_bindgen]
pub fn draw_ceiling_floor_raycast(
    position: JsValue,
    ceiling_floor_img: *mut u8,
    floor_texture: *mut u8,
    ceiling_texture: *mut u8,
    road_texture: *mut u8,
    ceiling_width_resolution: usize,
    ceiling_height_resolution: usize,
    ceiling_width_spacing: u8,
    ceiling_height_spacing: u8,
    height: usize,
    light_range: f32,
    map_light: f32,
    floor_texture_width: usize,
    floor_texture_height: usize,
    ceiling_texture_width: usize,
    ceiling_texture_height: usize,
    road_texture_width: usize,
    road_texture_height: usize,
    map_data: Vec<u8>,
    map_width: usize,
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
        let scale = ceiling_height_resolution as f32 / height as f32;
        let scaled_pitch = position.pitch * scale;
        let scaled_z = position.z * scale;

        let height_resolution_ratio =
            ceiling_height_resolution as f32 / ceiling_width_resolution as f32;
        let height_spacing_ratio = ceiling_height_spacing as f32 / ceiling_width_spacing as f32;
        let distance_divider =
            (2.0 * height_resolution_ratio) * (height_spacing_ratio) * position.plane_y_initial;
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

            let row_distance = cam_z / (p * distance_divider);
            let mut alpha = (row_distance + 0.0) / light_range - map_light;
            alpha = alpha.min(0.8);

            let floor_step_x = (row_distance * ray_dir_x_dist) / ceiling_width_resolution as f32;
            let floor_step_y = (row_distance * ray_dir_y_dist) / ceiling_width_resolution as f32;
            let mut floor_x = position.x + row_distance * ray_dir_x0;
            let mut floor_y = position.y + row_distance * ray_dir_y0;

            for x in 0..ceiling_width_resolution {
                floor_x += floor_step_x;
                floor_y += floor_step_y;

                // don't draw anything at values < 0
                if floor_x < 0.0 || floor_y < 0.0 {
                    continue;
                }

                let (is_of_value, value) = is_of_value_in_grid(
                    floor_x as i32,
                    floor_y as i32,
                    map_width as i32,
                    &map_data,
                    &[2, 3, 8, 9, 10, 11, 12, 13, 14, 15],
                );

                if !is_of_value {
                    continue;
                }

                // no ceiling for roads
                if !is_floor && [3, 12, 13, 14, 15].contains(&value) {
                    continue;
                }

                let (texture, texture_width, texture_height) =
                    if is_floor && [2, 8, 9, 10, 11].contains(&value) {
                        (floor_texture, floor_texture_width, floor_texture_height)
                    } else if is_floor && [3, 12, 13, 14, 15].contains(&value) {
                        (road_texture, road_texture_width, road_texture_height)
                    } else {
                        (
                            ceiling_texture,
                            ceiling_texture_width,
                            ceiling_texture_height,
                        )
                    };

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
    zbuffer_array: *mut f32,
    sprites_texture_array: *mut i32,
    sprites_texture_array_length: usize,
    light_range: f32,
    map_light: f32,
    width_resolution: usize,
    range: i32,
    map_array: *mut u8,
    map_width: i32,
    all_sprites_array: *mut f32,
    all_sprites_count: usize,
    found_sprites_array: *mut f32,
) -> JsValue {
    let found_sprites_length = raycast_visible_coordinates(
        position_js.clone(),
        100, // this really needs only enough to hit each square once
        range,
        map_array,
        map_width,
        all_sprites_array,
        all_sprites_count,
        found_sprites_array,
    );

    let mut stripe_parts: Vec<StripePart> = Vec::new();

    let position: Position = serde_wasm_bindgen::from_value(position_js).unwrap();
    let zbuffer = unsafe { std::slice::from_raw_parts(zbuffer_array, width_resolution) };
    let sprite_data =
        unsafe { std::slice::from_raw_parts(sprites_array, found_sprites_length * 5) };
    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    let mut sprites = Vec::new();
    for i in (0..found_sprites_length * 5).step_by(5) {
        sprites.push(Sprite {
            x: sprite_data[i],
            y: sprite_data[i + 1],
            angle: sprite_data[i + 2] as i32,
            height: sprite_data[i + 3] as i32,
            r#type: sprite_data[i + 4] as i32,
        });
    }

    sprites.sort_by(|a, b| {
        let da = (position.x - a.x).powi(2) + (position.y - a.y).powi(2);
        let db = (position.x - b.x).powi(2) + (position.y - b.y).powi(2);
        db.partial_cmp(&da).unwrap()
    });

    for sprite in sprites.iter() {
        // TODO: this is causing the first one to disappear??
        let (texture_height, texture_width) = texture_array
            .get(&sprite.r#type)
            .copied()
            .unwrap_or((100, 100));

        let aspect_ratio = texture_width as f32 / texture_height as f32;

        let projection = translate_coordinate_to_camera(
            position,
            sprite.x,
            sprite.y,
            sprite.height as f32 / 100.0,
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
                let z_index = ((stripe / width_spacing) as usize).clamp(0, width_resolution - 1);

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

#[wasm_bindgen]
pub fn draw_sprites_wasm1(array: *mut f32, array_length: usize) -> () {
    // no need to return antyhing
    // allow us to use the array
    let array_data = unsafe { std::slice::from_raw_parts_mut(array, array_length) };
    for value in array_data.iter_mut() {
        *value += 5.0;
    }
}
