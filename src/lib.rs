use helpers::{
    copy_to_raw_pointer, get_bits, get_grid_value, has_set_bits, parse_sprite_texture_array,
    Coords, Position, Sprite, StripePart, TranslationResult,
};
use js_sys::Math::atan2;
use wasm_bindgen::prelude::*;
mod helpers;
mod line_intersection;
use geo::Line;
use line_intersection::LineInterval;
use std::collections::HashMap;
use web_sys::console;
// let js: JsValue = vec![found_sprites_length as f32].into();
// console::log_2(&"Znj?".into(), &js);

pub fn raycast_column(
    column: i32,
    position: Position,
    map_data: &[u32],
    map_width: usize, // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
    coords: Option<&mut HashMap<String, Coords>>,
    sprites_map: Option<&mut HashMap<(i32, i32), Vec<[f32; 5]>>>,
    found_sprites_count: Option<&mut u32>,
    found_sprites: Option<&mut [f32]>,
) -> (f32, [i32; 7]) {
    // Use an empty HashMap if None is provided
    let mut default_coords = HashMap::new();
    let coords = coords.unwrap_or_else(|| &mut default_coords);
    let mut default_sprites_map = HashMap::new();
    let sprites_map = sprites_map.unwrap_or_else(|| &mut default_sprites_map);
    let found_sprites = found_sprites.unwrap_or_else(|| &mut []);

    // If found_sprites_count is None, use a local variable
    let mut found_sprites_dummy = 0;
    let found_sprites_count = found_sprites_count.unwrap_or(&mut found_sprites_dummy);

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
    let step_x: i8;
    let step_y: i8;
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
        let value = get_grid_value(map_x, map_y, map_width as i32, map_data);

        if has_set_bits(value, &[0], true) {
            hit_type = 1 as i8;
            // has door bit set
            if has_set_bits(value, &[0, 4, 5], true) {
                hit_type = value as i8;
            }

            // thin wall
            if has_set_bits(value, &[0, 4], true) {
                let has_set_north_bit = has_set_bits(value, &[6], false);
                let is_east = !has_set_north_bit;

                // from east or west side
                // offset is defined from the east or north
                let offset: f32;
                let distance_offset: f32;
                let bit_offset = get_bits(value, &[8, 9, 10, 11]);
                let bit_thickness = get_bits(value, &[12, 13, 14, 15]);
                let bit_width = get_bits(value, &[16, 17, 18, 19]);
                let bit_offset_secondary = get_bits(value, &[20, 21, 22, 23]);

                let offset1: f32 = (bit_offset % 11) as f32 / 10.0;
                let thickness: f32 = (bit_thickness % 11) as f32 / 10.0;
                let offset_secondary: f32 = (bit_offset_secondary % 11) as f32 / 10.0;
                let depth: f32 = (bit_width % 11) as f32 / 10.0;

                let ray_dirs: [f32; 2];
                let sides: [i32; 2];

                if is_east {
                    ray_dirs = [ray_dir_x, ray_dir_y];
                    sides = [0, 1];
                } else {
                    ray_dirs = [ray_dir_y, ray_dir_x];
                    sides = [1, 0];
                }

                if ray_dirs[0] <= 0.0 {
                    offset = offset1 + thickness;
                    // from east side
                    distance_offset = offset;
                } else {
                    offset = offset1;
                    distance_offset = 1.0 - offset;
                }

                let new_map_start_x;
                let new_map_end_x;
                let new_map_start_y;
                let new_map_end_y;

                // find the intersection of a line segment and an infinite line
                if is_east {
                    new_map_start_x = map_x as f32 + offset;
                    new_map_end_x = map_x as f32 + offset;
                    new_map_start_y = map_y as f32 + offset_secondary;
                    new_map_end_y = map_y as f32 + offset_secondary + (depth);
                } else {
                    new_map_start_y = map_y as f32 + offset;
                    new_map_end_y = map_y as f32 + offset;
                    new_map_start_x = map_x as f32 + offset_secondary;
                    new_map_end_x = map_x as f32 + offset_secondary + (depth);
                }

                // the segment of line at the offset of the wall
                let segment = LineInterval::line_segment(Line {
                    start: (new_map_start_x as f32, new_map_start_y as f32).into(),
                    end: (new_map_end_x as f32, new_map_end_y as f32).into(),
                });

                let segment_map_adder;
                // the segment of line between the offsets of the wall
                if ray_dirs[1] > 0.0 {
                    // depending on which side we're looking at the space between the offsets from
                    segment_map_adder = offset_secondary + 0.0;
                } else {
                    segment_map_adder = offset_secondary + (depth);
                }

                let new_map_between_start_x;
                let new_map_between_end_x;
                let new_map_between_start_y;
                let new_map_between_end_y;

                if is_east {
                    new_map_between_start_x = map_x as f32 + offset1;
                    new_map_between_end_x = map_x as f32 + offset1 + thickness;
                    new_map_between_start_y = map_y as f32 + segment_map_adder;
                    new_map_between_end_y = map_y as f32 + segment_map_adder;
                } else {
                    new_map_between_start_y = map_y as f32 + offset1;
                    new_map_between_end_y = map_y as f32 + offset1 + thickness;
                    new_map_between_start_x = map_x as f32 + segment_map_adder;
                    new_map_between_end_x = map_x as f32 + segment_map_adder;
                }

                // the segment of line between the offsets of the wall
                let segment_between = LineInterval::line_segment(Line {
                    start: (
                        new_map_between_start_x as f32,
                        new_map_between_start_y as f32,
                    )
                        .into(),
                    end: (new_map_between_end_x as f32, new_map_between_end_y as f32).into(),
                });

                // ray between player position and point on the ray direction
                let line = LineInterval::ray(Line {
                    start: (position.x as f32, position.y as f32).into(),
                    end: (position.x + ray_dir_x as f32, position.y + ray_dir_y as f32).into(),
                });

                // check main segment line
                let intersection = segment.relate(&line).unique_intersection();
                if let Some(_) = intersection {
                    hit = 1;
                    // move it back for the amount it should move back (assign to both even though only 1 will be used, x for east/west and y for north/south)

                    side_dist_x += delta_dist_x * (1.0 - (distance_offset));
                    side_dist_y += delta_dist_y * (1.0 - (distance_offset));

                    side = sides[0];
                } else {
                    // check line between segments of thickness
                    let intersection_between = segment_between.relate(&line).unique_intersection();
                    if let Some(_) = intersection_between {
                        hit = 1;
                        side = sides[1];
                        hit_type = 1; // show wall

                        if ray_dirs[1] < 0.0 {
                            // move it back for the amount it should move back due to depth
                            // if we're looking at it from the shortened side
                            side_dist_y += delta_dist_y * (1.0 - depth);
                            side_dist_x += delta_dist_x * (1.0 - depth);

                            // move it forward for the amount it should move forward due to secondary offset
                            side_dist_y -= delta_dist_y * (offset_secondary);
                            side_dist_x -= delta_dist_x * (offset_secondary);
                        } else {
                            // move it back for the amount it should move back due to secondary offset
                            side_dist_y += delta_dist_y * (offset_secondary);
                            side_dist_x += delta_dist_x * (offset_secondary);
                        }
                    }
                }
            }

            if value == 1 {
                hit = 1;
            }
        }

        // add in sprites from the coordinate in the way, if we haven't already
        let coord_key = format!("{}-{}", map_x, map_y);
        if !coords.contains_key(&coord_key) {
            coords.insert(coord_key.clone(), Coords { x: map_x, y: map_y });

            if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
                for &sprite in sprite_list {
                    let index = (*found_sprites_count as usize) * 5; // Convert u32 to usize

                    found_sprites[index..index + 5].copy_from_slice(&sprite);
                    *found_sprites_count += 1;
                }
            }
        }

        // don't do any more coordinate increments if hit
        if hit == 1 {
            break;
        }

        // jump to next map square, either in x-direction, or in y-direction;
        // post-increment so we don't miss out on content in the immediate coordinate we're standing in
        if side_dist_x < side_dist_y {
            side_dist_x += delta_dist_x;
            map_x += step_x as i32;
            side = 0;
        } else {
            side_dist_y += delta_dist_y;
            map_y += step_y as i32;
            side = 1;
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

    (
        perp_wall_dist,
        [
            tex_x,
            left,
            draw_start_y as i32,
            wall_height,
            (global_alpha * 100.0) as i32,
            hit as i32,
            hit_type as i32,
        ],
    )
}

#[wasm_bindgen]
pub fn draw_walls_raycast(
    columns_array: *mut i32,
    zbuffer_array: *mut f32,
    position: JsValue,
    map_array: *mut u32, // 2D array representing the grid map
    map_width: usize,    // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
    found_sprites_array: *mut f32,
    all_sprites_array: *mut f32,
    all_sprites_count: usize,
) -> u32 {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let map_data =
        unsafe { std::slice::from_raw_parts(map_array, (map_width * map_width) as usize) };
    let all_sprites_data =
        unsafe { std::slice::from_raw_parts(all_sprites_array, all_sprites_count * 5) };
    let mut found_sprites =
        unsafe { std::slice::from_raw_parts_mut(found_sprites_array, all_sprites_count * 5) };

    let mut coords: HashMap<String, Coords> = HashMap::new();
    let mut sprites_map: HashMap<(i32, i32), Vec<[f32; 5]>> = HashMap::new();
    let mut found_sprites_count = 0;

    // map them by x & y coordinate for easy access
    for i in (0..all_sprites_count * 5).step_by(5) {
        let sprite_data: [_; 5] = all_sprites_data[i..i + 5].try_into().unwrap();

        let key = (sprite_data[0].floor() as i32, sprite_data[1].floor() as i32);

        sprites_map
            .entry(key)
            .or_insert_with(Vec::new)
            .push(sprite_data);
    }

    for column in 0..width_resolution {
        let (perp_wall_dist, col_data) = raycast_column(
            column,
            position,
            map_data,
            map_width,
            width_resolution,
            height,
            width,
            width_spacing,
            light_range,
            range,
            wall_texture_width,
            Some(&mut coords),
            Some(&mut sprites_map),
            Some(&mut found_sprites_count),
            Some(&mut found_sprites),
        );

        copy_to_raw_pointer(columns_array, 8 * column as usize, &col_data);
        copy_to_raw_pointer(zbuffer_array, column as usize, &[perp_wall_dist]);
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
    map_array: *mut u32,
    map_width: usize,
) -> () {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let map_data =
        unsafe { std::slice::from_raw_parts(map_array, (map_width * map_width) as usize) };

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

                let value =
                    get_grid_value(floor_x as i32, floor_y as i32, map_width as i32, map_data);

                let has_set_any_bits = has_set_bits(
                    value,
                    &[1, 2, 3], // ceiling, floor or road
                    false,
                );

                if !has_set_any_bits {
                    continue;
                }

                let has_set_ceiling_bit = has_set_bits(value, &[2], false);
                let has_set_floor_bit = has_set_bits(
                    value,
                    &[1], // ceiling, floor or road
                    false,
                );
                let has_set_road_bit = has_set_bits(
                    value,
                    &[3], // ceiling, floor or road
                    false,
                );

                // no ceiling for roads
                if !is_floor && !has_set_ceiling_bit {
                    continue;
                }

                let (texture, texture_width, texture_height) = if is_floor && has_set_road_bit {
                    (road_texture, road_texture_width, road_texture_height)
                } else if is_floor && has_set_floor_bit {
                    (floor_texture, floor_texture_width, floor_texture_height)
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
    map_array: *mut u32,
    map_width: i32,
    all_sprites_array: *mut f32,
    all_sprites_count: usize,
    found_sprites_array: *mut f32,
    found_sprites_count: u32,
) -> JsValue {
    let found_sprites_length = found_sprites_count as usize;

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

// move if no wall in front of you
#[wasm_bindgen]
pub fn walk(
    position_js: JsValue,
    distance: f32,
    map_array: *mut u32,
    map_width: i32,
    width_resolution: usize,
    width: i32,
    height: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
) -> JsValue {
    let position: Position = serde_wasm_bindgen::from_value(position_js).unwrap();
    let map_data =
        unsafe { std::slice::from_raw_parts(map_array, (map_width * map_width) as usize) };

    let mut raycast_position = position.clone();
    // check behind you by turning
    if distance < 0.0 {
        raycast_position.dir_x = position.dir_x * -1.0;
        raycast_position.dir_y = position.dir_y * -1.0;
    }

    // raycast middle column to get the distance
    let (perp_wall_dist, col_data) = raycast_column(
        (width_resolution / 2) as i32,
        raycast_position,
        map_data,
        map_width as usize,
        width_resolution as i32,
        height,
        width,
        width_spacing,
        light_range,
        range,
        wall_texture_width,
        None,
        None,
        None,
        None,
    );

    let mut x = position.x;
    let mut y = position.y;

    // if far enough or not a wall
    if perp_wall_dist > 0.2 || col_data[6] != 1 {
        x += position.dir_x * distance;
        y += position.dir_y * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    let mut raycast_position_x = raycast_position.clone();
    raycast_position_x.dir_y = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_x, _) = raycast_column(
        (width_resolution / 2) as i32,
        raycast_position_x,
        map_data,
        map_width as usize,
        width_resolution as i32,
        height,
        width,
        width_spacing,
        light_range,
        range,
        wall_texture_width,
        None,
        None,
        None,
        None,
    );
    if perp_wall_dist_x > 0.2 {
        x += position.dir_x * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    // if we weren't able to move x, check if we can move y
    let mut raycast_position_y = raycast_position.clone();
    // raycast_position_y.y = y + position.dir_y * distance;
    raycast_position_y.dir_x = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_y, _) = raycast_column(
        (width_resolution / 2) as i32,
        raycast_position_y,
        map_data,
        map_width as usize,
        width_resolution as i32,
        height,
        width,
        width_spacing,
        light_range,
        range,
        wall_texture_width,
        None,
        None,
        None,
        None,
    );
    if perp_wall_dist_y > 0.2 {
        y += position.dir_y * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    serde_wasm_bindgen::to_value(&vec![x, y]).unwrap()
}
