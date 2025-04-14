#![feature(map_try_insert)]

use helpers::{
    copy_to_raw_pointer, get_bits, get_grid_value, has_set_bits, parse_sprite_texture_array,
    Coords, Position, Sprite, SpritePart, TranslationResult,
};
use js_sys::Math::atan2;
use wasm_bindgen::prelude::*;
mod helpers;
mod line_intersection;
use geo::{Coord, HausdorffDistance, Line};
use line_intersection::LineInterval;
use std::collections::HashSet;
use std::default;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use std::{collections::HashMap, f32::MAX};
use web_sys::console;
// let js: JsValue = vec![found_sprites_length as f32].into();
// console::log_2(&"Znj?".into(), &js);
use core::array::from_fn;
use std::ptr::write_bytes;
use std::slice::from_raw_parts;
use std::slice::from_raw_parts_mut;
// pub use wasm_bindgen_rayon::init_thread_pool;

pub fn raycast_column(
    column: i32,
    position: Position,
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
    // coords: Option<&Arc<Mutex<HashMap<std::string::String, Coords>>>>,
    sprites_map: Option<&HashMap<(i32, i32), Vec<[f32; 5]>>>,
    // found_sprites_count: Option<&Arc<Mutex<u32>>>,
    // found_sprites: Option<&Arc<Mutex<&mut [f32]>>>,
    skip_sprites_and_writes: bool,
    // columns_array: Option<&Arc<Mutex<&mut [i32]>>>,
    // zbuffer_array: Option<&Arc<Mutex<&mut [f32]>>>,
    stop_at_window: bool,
) -> (f32, [i32; 7], Vec<(i32, i32)>, Vec<[f32; 9]>) {
    let mut met_coords: HashMap<(i32, i32), i32> = HashMap::new();
    let mut window_sprites: Vec<[f32; 9]> = vec![];

    // Use an empty HashMap if None is provided
    // let mut default_coords = Arc::new(Mutex::new(HashMap::new()));
    // let coords = coords.unwrap_or_else(|| &default_coords);

    let mut default_sprites_map = HashMap::new();
    let sprites_map = sprites_map.unwrap_or_else(|| &mut default_sprites_map);

    // let mut default_found_sprites: Arc<Mutex<&mut [f32]>> = Arc::new(Mutex::new(&mut []));
    // let found_sprites = found_sprites.unwrap_or_else(|| &mut default_found_sprites);

    // let mut default_zbuffer: Arc<Mutex<&mut [f32]>> = Arc::new(Mutex::new(&mut []));
    // let zbuffer = zbuffer_array.unwrap_or_else(|| &mut default_zbuffer);

    // let mut default_columns: Arc<Mutex<&mut [i32]>> = Arc::new(Mutex::new(&mut []));
    // let columns = columns_array.unwrap_or_else(|| &mut default_columns);

    let mut calculated_texture_width: i32 = wall_texture_width;

    // If found_sprites_count is None, use a local variable
    // let found_sprites_dummy: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
    // let found_sprites_count = found_sprites_count.unwrap_or_else(|| &found_sprites_dummy);

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
    let mut wall_width = 1.0;
    let mut wall_offset = 0.0;

    while hit == 0 && remaining_range >= 0 {
        let value: u64 = get_grid_value(map_x, map_y, map_width as i32, map_data);

        // if wall bit is set
        if has_set_bits(value, &[0], true) {
            hit_type = 1 as i8;

            let is_doors = [
                has_set_bits(value, &[5], true),
                has_set_bits(value, &[4], true),
                has_set_bits(value, &[4], true),
            ];

            let is_windows = [has_set_bits(value, &[8], true), false, false];

            let initial_bit_offset = 16;
            let bit_offsets = [
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 16 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 32 + i as u8),
                ),
            ];
            let bit_thicknesses = [
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 4 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 20 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 36 + i as u8),
                ),
            ];
            let bit_widths = [
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 8 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 24 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 40 + i as u8),
                ),
            ];
            let bit_offset_secondaries = [
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 12 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 28 + i as u8),
                ),
                get_bits(
                    value,
                    &from_fn::<u8, 4, _>(|i| initial_bit_offset + 44 + i as u8),
                ),
            ];
            let has_set_north_bits = [
                has_set_bits(value, &[6], false),
                has_set_bits(value, &[7], false),
                has_set_bits(value, &[2], false),
            ];
            let mut coord_delta_dist_x = MAX;
            let mut coord_delta_dist_y = MAX;
            let mut distance = MAX;
            let mut local_width: f32 = 1.0;
            let mut local_offset: f32 = 1.0;

            // we support two lines per coordinate
            for i in 0..3 {
                // no shenanigans if the thickness is 0, we'll allow width to be 0 for e.g. windows
                if bit_widths[i] == 0 {
                    continue;
                }
                let is_east = !has_set_north_bits[i];
                let is_door = is_doors[i];
                let is_window = is_windows[i];

                let mut local_delta_dist_x = 0.0;
                let mut local_delta_dist_y = 0.0;

                // from east or west side
                // offset is defined from the east or north
                let offset: f32;
                let distance_offset: f32;

                let offset1: f32 = (bit_offsets[i] % 11) as f32 / 10.0;
                let thickness: f32 = (bit_thicknesses[i] % 11) as f32 / 10.0;
                let offset_secondary: f32 = (bit_offset_secondaries[i] % 11) as f32 / 10.0;
                let depth: f32 = (bit_widths[i] % 11) as f32 / 10.0;

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
                    new_map_end_y = map_y as f32 + offset_secondary + depth;
                } else {
                    new_map_start_y = map_y as f32 + offset;
                    new_map_end_y = map_y as f32 + offset;
                    new_map_start_x = map_x as f32 + offset_secondary;
                    new_map_end_x = map_x as f32 + offset_secondary + depth;
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
                    segment_map_adder = offset_secondary;
                } else {
                    segment_map_adder = offset_secondary + depth;
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
                let mut local_hit = 0;
                let mut local_side = 0;
                let mut local_intersection_coord: Coord<f32> = Coord::zero();
                if let Some(coord) = intersection {
                    local_intersection_coord = coord;
                    local_hit = 1;
                    // move it back for the amount it should move back (assign to both even though only 1 will be used, x for east/west and y for north/south)

                    local_delta_dist_x += delta_dist_x * (1.0 - (distance_offset));
                    local_delta_dist_y += delta_dist_y * (1.0 - (distance_offset));

                    local_side = sides[0];

                    // since we'd like texture to match the width if it's a door
                    if is_door || is_window {
                        local_width = depth;
                        local_offset = offset_secondary;
                    } else {
                        local_width = 1.0;
                        local_offset = 0.0;
                    }
                } else {
                    // check line between segments of thickness
                    let intersection_between = segment_between.relate(&line).unique_intersection();
                    if let Some(coord) = intersection_between {
                        local_intersection_coord = coord;
                        local_hit = 1;
                        local_side = sides[1];
                        hit_type = 1; // show wall even if door since this is the side

                        // no texture x snehaningans from the sides
                        local_width = 1.0;
                        local_offset = 0.0;

                        if ray_dirs[1] < 0.0 {
                            // move it back for the amount it should move back due to depth
                            // if we're looking at it from the shortened side
                            local_delta_dist_y += delta_dist_y * (1.0 - depth);
                            local_delta_dist_x += delta_dist_x * (1.0 - depth);

                            // move it forward for the amount it should move forward due to secondary offset
                            local_delta_dist_y -= delta_dist_y * (offset_secondary);
                            local_delta_dist_x -= delta_dist_x * (offset_secondary);
                        } else {
                            // move it back for the amount it should move back due to secondary offset
                            local_delta_dist_y += delta_dist_y * (offset_secondary);
                            local_delta_dist_x += delta_dist_x * (offset_secondary);
                        }
                    }
                }
                if local_hit == 1 {
                    // take the shortest of the two paths
                    let local_distance = local_intersection_coord
                        .hausdorff_distance(&[Coord::from([position.x, position.y])]);
                    if local_distance < distance {
                        distance = local_distance;
                        coord_delta_dist_x = local_delta_dist_x;
                        coord_delta_dist_y = local_delta_dist_y;
                        side = local_side;
                        hit = 1;
                        wall_width = local_width;
                        wall_offset = local_offset;
                        // has door bit set
                        if is_door {
                            hit_type = 0x2 as i8;
                        } else if is_window {
                            hit_type = 0x3 as i8;
                            // keep going but add to sprites array
                            if !stop_at_window {
                                hit = 0;
                            }

                            // add to visible sprites
                            if !skip_sprites_and_writes {
                                let window_data = [
                                    local_intersection_coord.x,
                                    local_intersection_coord.y,
                                    0.0,
                                    100.0,
                                    7 as f32,
                                    column as f32,
                                    side as f32,
                                    wall_offset,
                                    wall_width,
                                ];

                                window_sprites.push(window_data);
                                // let mut unlocked_found_sprites_count =
                                //     found_sprites_count.lock().unwrap();
                                // let index = ((*unlocked_found_sprites_count) as usize) * 9; // Convert u32 to usize

                                // let mut unlocked_found_sprites = found_sprites.lock().unwrap();

                                // (*unlocked_found_sprites)[index..index + 9].copy_from_slice(
                                //     &window_data, // x, y, angle (0-360), height (multiplier of 1 z), type, column, side, offset, width
                                // );
                                // // let js: JsValue = vec![*found_sprites_count as f32].into();
                                // // console::log_2(&"Znj?".into(), &js);
                                // *unlocked_found_sprites_count += 1;
                            }
                        } else {
                            hit_type = 1;
                        }
                    }
                }
            }
            if hit == 1 {
                side_dist_x += coord_delta_dist_x;
                side_dist_y += coord_delta_dist_y;
                calculated_texture_width = (wall_texture_width as f32) as i32;
            }
        }

        // handle thick wall
        if value == 1 {
            wall_width = 1.0;
            wall_offset = 0.0;
            hit = 1;
        }

        // only add coord if sprites exist in it
        if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
            let _ = met_coords.try_insert((map_x, map_y), 0);
        }
        // add in sprites from the coordinate in the way, if we haven't already
        // let coord_key = format!("{}-{}", map_x, map_y);
        // let mut unlocked_coords = coords.lock().unwrap();
        // if !unlocked_coords.contains_key(&coord_key) {
        //     (*unlocked_coords).insert(coord_key.clone(), Coords { x: map_x, y: map_y });

        //     if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
        //         for &sprite in sprite_list {
        //             let mut unlocked_found_sprites_count = found_sprites_count.lock().unwrap();
        //             let index = (*unlocked_found_sprites_count as usize) * 9; // Convert u32 to usize

        //             let mut unlocked_found_sprites = found_sprites.lock().unwrap();
        //             (*unlocked_found_sprites)[index..index + 5].copy_from_slice(&sprite);
        //             *unlocked_found_sprites_count += 1;
        //         }
        //     }
        // }

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
    // scale the height according to width so they're square!
    let line_height = width as f32 / 2.0 / perp_wall_dist;
    let aspect_ratio = height as f32 / width as f32;

    let draw_start_y = -line_height / 2.0
        + height as f32 / 2.0
        + position.pitch
        + position.z / (perp_wall_dist * (2.0 * aspect_ratio));
    let draw_end_y = line_height / 2.0
        + height as f32 / 2.0
        + position.pitch
        + position.z / (perp_wall_dist * (2.0 * aspect_ratio));

    wall_x -= wall_x.floor();

    // since we'd like texture to match the width if it's a door
    wall_x -= wall_offset;
    wall_x /= wall_width;

    let tex_x = (wall_x * calculated_texture_width as f32) as i32;
    let tex_x = if side == 0 && ray_dir_x > 0.0 {
        calculated_texture_width - tex_x - 1
    } else {
        tex_x
    };
    let tex_x = if side == 1 && ray_dir_y < 0.0 {
        calculated_texture_width - tex_x - 1
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

    let col_data = [
        tex_x,
        left,
        draw_start_y as i32,
        wall_height,
        (global_alpha * 100.0) as i32,
        hit as i32,
        hit_type as i32,
    ];

    // if !skip_sprites_and_writes {
    //     let mut unlocked_columns = columns.lock().unwrap();
    //     (*unlocked_columns)[8 * column as usize..(8 * column + 7) as usize]
    //         .copy_from_slice(&col_data);
    //     // copy_to_raw_pointer(columns, 8 * column as usize, &col_data);
    //     let mut unlocked_zbuffer = zbuffer.lock().unwrap();
    //     (*unlocked_zbuffer)[column as usize] = perp_wall_dist;
    // }

    (
        perp_wall_dist,
        col_data,
        met_coords.keys().cloned().collect(),
        window_sprites,
    )
}

#[wasm_bindgen]
pub fn draw_walls_raycast(
    columns_array: *mut i32, // TODO: should these be pointers to arrays??
    zbuffer_array: *mut f32,
    position: JsValue,
    map_array: *mut u64, // 2D array representing the grid map
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
    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };
    let all_sprites_data = unsafe { from_raw_parts(all_sprites_array, all_sprites_count * 5) };
    let found_sprites = unsafe {
        from_raw_parts_mut(
            found_sprites_array,
            (all_sprites_count + (2 * width_resolution) as usize) * 9,
        )
    };
    let zbuffer = unsafe { from_raw_parts_mut(zbuffer_array, width_resolution as usize) };
    let columns = unsafe { from_raw_parts_mut(columns_array, (8 * width_resolution) as usize) }; // TODO: is this not too much??

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

    // for column in 0..width_resolution {
    //     raycast_column(
    //         column,
    //         position,
    //         map_data,
    //         map_width,
    //         width_resolution,
    //         height,
    //         width,
    //         width_spacing,
    //         light_range,
    //         range,
    //         wall_texture_width,
    //         Some(&coords),
    //         Some(&mut sprites_map),
    //         Some(&found_sprites_count),
    //         Some(&found_sprites),
    //         false,
    //         Some(&mut columns),
    //         Some(&zbuffer),
    //         false,
    //     );
    // }

    // let start = Instant::now();

    let mut data: Vec<(f32, [i32; 7], Vec<(i32, i32)>, Vec<[f32; 9]>)> = (0..width_resolution)
        .into_par_iter()
        .map(|column| {
            let (perp_wall_dist, col_data, met_coords, window_sprites) = raycast_column(
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
                // Some(&coords),
                Some(&sprites_map),
                // Some(&found_sprites_count),
                // Some(&found_sprites),
                false,
                // Some(&columns),
                // Some(&zbuffer),
                false,
            );

            (perp_wall_dist, col_data, met_coords, window_sprites)
        })
        .collect();

    // let elapsed = start.elapsed().as_millis() as u32;
    // let js: JsValue = vec![elapsed].into();
    // console::log_2(&"Znj?".into(), &js);

    let mut all_met_coords: Vec<(i32, i32)> = vec![];
    for (_, _, met_coords, _) in data.iter_mut() {
        all_met_coords.append(met_coords);
    }
    // let js: JsValue = vec![all_met_coords.len() as f32].into();
    // console::log_2(&"Znj?".into(), &js);
    let uniqued_met_coords = all_met_coords
        .into_iter()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    // let js: JsValue = vec![uniqued_met_coords.len() as f32].into();
    // console::log_2(&"Znj?1".into(), &js);
    for (x, y) in uniqued_met_coords {
        let (map_x, map_y) = (x as i32, y as i32);
        // let coord_key = format!("{}-{}", map_x, map_y);
        // if !coords.contains_key(&coord_key) {
        // coords.insert(coord_key.clone(), Coords { x: map_x, y: map_y });

        if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
            for &sprite in sprite_list {
                let index = (found_sprites_count as usize) * 9; // Convert u32 to usize

                (found_sprites)[index..index + 5].copy_from_slice(&sprite);
                found_sprites_count += 1;
            }
        }
        // }
    }

    for (column, (perp_wall_dist, col_data, _, window_sprites)) in data.iter().enumerate() {
        (columns)[8 * column as usize..(8 * column + 7) as usize].copy_from_slice(col_data);
        (zbuffer)[column as usize] = *perp_wall_dist;

        // for (x, y) in met_coords {
        //     let (map_x, map_y) = (*x as i32, *y as i32);
        //     let coord_key = format!("{}-{}", map_x, map_y);
        //     if !coords.contains_key(&coord_key) {
        //         coords.insert(coord_key.clone(), Coords { x: map_x, y: map_y });

        //         if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
        //             for &sprite in sprite_list {
        //                 let index = (found_sprites_count as usize) * 9; // Convert u32 to usize

        //                 (found_sprites)[index..index + 5].copy_from_slice(&sprite);
        //                 found_sprites_count += 1;
        //             }
        //         }
        //     }
        // }

        for window_sprite in window_sprites {
            let index = ((found_sprites_count) as usize) * 9; // Convert u32 to usize

            (found_sprites)[index..index + 9].copy_from_slice(
                window_sprite, // x, y, angle (0-360), height (multiplier of 1 z), type, column, side, offset, width
            );
            found_sprites_count += 1;
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
    map_array: *mut u64,
    map_width: usize,
) -> () {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };

    let road_texture_array = unsafe {
        from_raw_parts(
            road_texture,
            (road_texture_width * road_texture_height * 4) as usize,
        )
    };
    let ceiling_texture_array = unsafe {
        from_raw_parts(
            ceiling_texture,
            (ceiling_texture_width * ceiling_texture_height * 4) as usize,
        )
    };
    let floor_texture_array = unsafe {
        from_raw_parts(
            floor_texture,
            (floor_texture_width * floor_texture_height * 4) as usize,
        )
    };

    unsafe {
        // blank out the whole image buffer
        write_bytes(
            ceiling_floor_img,
            0,
            ceiling_width_resolution * ceiling_height_resolution * 4,
        );
    }
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

    let outer_data: Vec<Vec<(i32, [u8; 4])>> = (0..ceiling_height_resolution)
        .into_par_iter()
        .map(|y| {
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

            let data: Vec<(i32, [u8; 4])> = (0..ceiling_width_resolution)
                .into_iter()
                .map(|x| {
                    // });
                    // for x in 0..ceiling_width_resolution {
                    floor_x += floor_step_x;
                    floor_y += floor_step_y;

                    // don't draw anything at values < 0
                    if floor_x < 0.0 || floor_y < 0.0 {
                        return (-1, [0, 0, 0, 0]);
                    }

                    let value =
                        get_grid_value(floor_x as i32, floor_y as i32, map_width as i32, map_data);

                    let has_set_any_bits = has_set_bits(
                        value,
                        &[1, 3], // ceiling, floor or road
                        false,
                    );

                    if !has_set_any_bits {
                        return (-1, [0, 0, 0, 0]);
                    }

                    let has_set_ceiling_bit = has_set_bits(value, &[1], false);
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
                        return (-1, [0, 0, 0, 0]);
                    }

                    let (texture, texture_width, texture_height) = if is_floor && has_set_road_bit {
                        (road_texture_array, road_texture_width, road_texture_height)
                    } else if is_floor && has_set_floor_bit {
                        (
                            floor_texture_array,
                            floor_texture_width,
                            floor_texture_height,
                        )
                    } else {
                        (
                            ceiling_texture_array,
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

                    let darkening_factor = 1.0 - alpha; // Adjust for the desired darkness
                    let r = (texture[tex_idx] as f32 * darkening_factor) as u8;
                    let g = (texture[tex_idx + 1] as f32 * darkening_factor) as u8;
                    let b = (texture[tex_idx + 2] as f32 * darkening_factor) as u8;

                    (pixel_idx as i32, [r, g, b, 255])
                })
                .collect();

            data
        })
        .collect();

    for data in outer_data {
        for (pixel_idx, rgb_array) in data {
            if pixel_idx >= 0 {
                copy_to_raw_pointer(
                    ceiling_floor_img,
                    pixel_idx as usize,
                    &[rgb_array[0], rgb_array[1], rgb_array[2], rgb_array[3]],
                );
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
    let aspect_ratio = height as f32 / width as f32;
    let v_move_screen = position.pitch + (position.z) / (transform_y * (aspect_ratio * 2.0));

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
        screen_x, // TODO: to i32??
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
    visible_sprites_array: *mut f32,
    sprite_parts_array: *mut i32,
    zbuffer_array: *mut f32,
    sprites_texture_array: *mut i32,
    sprites_texture_array_length: usize,
    light_range: f32,
    map_light: f32,
    width_resolution: usize,
    found_sprites_count: u32,
    all_sprites_count: u32,
) -> usize {
    let found_sprites_length = found_sprites_count as usize;

    // let mut sprite_parts: Vec<SpritePart> = Vec::new();

    let position: Position = serde_wasm_bindgen::from_value(position_js).unwrap();
    let zbuffer = unsafe { from_raw_parts(zbuffer_array, width_resolution) };
    let sprite_data = unsafe { from_raw_parts(visible_sprites_array, found_sprites_length * 9) };
    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    let mut sprites = Vec::new();
    // copy them since we need to sort them anyway
    for i in (0..found_sprites_length * 9).step_by(9) {
        sprites.push(Sprite {
            x: sprite_data[i],
            y: sprite_data[i + 1],
            angle: sprite_data[i + 2] as i32,
            height: sprite_data[i + 3] as i32,
            r#type: sprite_data[i + 4] as i32,
            column: sprite_data[i + 5] as u32,
            side: sprite_data[i + 6] as u8,
            offset: sprite_data[i + 7],
            width: sprite_data[i + 8],
        });
    }

    // since we should draw those in the distance first, we sort them
    sprites.sort_by(|a, b| {
        let da = (position.x - a.x).powi(2) + (position.y - a.y).powi(2);
        let db = (position.x - b.x).powi(2) + (position.y - b.y).powi(2);
        db.partial_cmp(&da).unwrap()
    });

    let sprite_parts_collected: Vec<Vec<SpritePart>> = sprites
        .into_par_iter()
        .map(|sprite| {
            let mut sprite_parts_inner: Vec<SpritePart> = Vec::new();

            let projection = translate_coordinate_to_camera(
                position,
                sprite.x,
                sprite.y,
                sprite.height as f32 / 100.0,
                width,
                height,
            );

            let alpha = projection.distance / light_range - map_light;
            // ensure sprites are always at least a little bit visible - alpha 1 is all black
            let alpha_i = (100.0 - alpha * 100.0).floor().clamp(20.0, 100.0) as i32;

            // TODO: this is causing the first one to disappear??
            let (texture_height, texture_width) = texture_array
                .get(&sprite.r#type)
                .copied()
                .unwrap_or((100, 100));

            if sprite.r#type == 7 {
                // switch which side we were raycasting from to take the fract part to know where the texture was hit
                let mut fract: f32;
                // TODO: maybe not keep all of this in memory and just pass the fract around?
                if sprite.side == 1 {
                    fract = sprite.x.abs().fract();
                } else {
                    fract = sprite.y.abs().fract();
                }
                // since we'd like the texture to match the width
                fract -= sprite.offset;
                fract /= sprite.width;

                let texture_x: i32 = (fract * texture_width as f32) as i32;
                sprite_parts_inner.push(SpritePart {
                    sprite_type: sprite.r#type,
                    sprite_left_x: sprite.column as i32,
                    sprite_right_x: sprite.column as i32 + width_spacing,
                    screen_y_ceiling: projection.screen_y_ceiling as i32,
                    screen_y_floor: projection.screen_y_floor as i32,
                    tex_x1: texture_x,
                    tex_x2: (1.0
                        + (width_spacing as f32 / width_resolution as f32) * texture_width as f32
                        + texture_x as f32) as i32,
                    alpha: alpha_i,
                    angle: 0,
                });
                return sprite_parts_inner;
            }

            let aspect_ratio = texture_width as f32 / texture_height as f32;

            let dx = position.x - sprite.x;
            let dy = position.y - sprite.y;
            let angle = atan2(dx as f64, dy as f64);
            // will return from -180 to 180
            let angle_i = (((angle).to_degrees() as i32) + 180 + sprite.angle) % 360;

            let sprite_width = (projection.full_height * aspect_ratio as f32).abs() as i32;

            let draw_start_x = (-sprite_width as f32 / 2.0 + projection.screen_x).max(0.0) as i32;
            let draw_end_x =
                (sprite_width as f32 / 2.0 + projection.screen_x).min(width as f32 - 1.0) as i32;

            let mut sprite_parts_temp = Vec::new();
            for stripe in (draw_start_x..draw_end_x).step_by(width_spacing as usize) {
                if projection.distance > 0.0 && stripe >= 0 && stripe < width {
                    let z_index =
                        ((stripe / width_spacing) as usize).clamp(0, width_resolution - 1);

                    if projection.distance < zbuffer[z_index] {
                        if sprite_parts_temp.len() % 2 == 0 {
                            sprite_parts_temp.push(stripe);
                        }
                        if stripe + width_spacing >= draw_end_x && sprite_parts_temp.len() % 2 == 1
                        {
                            sprite_parts_temp.push(stripe);
                        }
                    } else if sprite_parts_temp.len() % 2 == 1 {
                        sprite_parts_temp.push(stripe);
                    }
                }
            }

            for pair in sprite_parts_temp.chunks_exact(2) {
                let sprite_width_f64 = sprite_width as f64;
                let screen_x_f64 = projection.screen_x as f64;

                let tex_x1 = (((pair[0] as f64 - (-sprite_width_f64 / 2.0 + screen_x_f64))
                    * texture_width as f64)
                    / sprite_width_f64) as i32;
                let tex_x2 = (((pair[1] as f64 - (-sprite_width_f64 / 2.0 + screen_x_f64))
                    * texture_width as f64)
                    / sprite_width_f64) as i32;

                sprite_parts_inner.push(SpritePart {
                    sprite_type: sprite.r#type,
                    sprite_left_x: pair[0],
                    sprite_right_x: pair[1],
                    screen_y_ceiling: projection.screen_y_ceiling as i32,
                    screen_y_floor: projection.screen_y_floor as i32,
                    tex_x1,
                    tex_x2,
                    alpha: alpha_i,
                    angle: angle_i,
                });
            }

            sprite_parts_inner
        })
        .collect();
    let sprite_parts_flattened: Vec<SpritePart> =
        sprite_parts_collected.into_iter().flatten().collect();

    for i in 0..sprite_parts_flattened.len() {
        let sprite = &sprite_parts_flattened[i];
        copy_to_raw_pointer(
            sprite_parts_array,
            9 * i,
            &[
                sprite.sprite_type,
                sprite.sprite_left_x,
                sprite.sprite_right_x,
                sprite.screen_y_ceiling,
                sprite.screen_y_floor,
                sprite.tex_x1,
                sprite.tex_x2,
                sprite.alpha,
                sprite.angle,
            ],
        );
    }

    sprite_parts_flattened.len()
}

// move if no wall in front of you
#[wasm_bindgen]
pub fn walk(
    position_js: JsValue,
    distance: f32,
    map_array: *mut u64,
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
    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };

    let mut raycast_position = position.clone();

    // check behind you by turning
    if distance < 0.0 {
        raycast_position.dir_x = position.dir_x * -1.0;
        raycast_position.dir_y = position.dir_y * -1.0;
    }

    // raycast middle column to get the distance
    let (perp_wall_dist, col_data, _, _) = raycast_column(
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
        // None,
        None,
        // None,
        // None,
        true,
        // None,
        // None,
        true,
    );

    let mut x = position.x;
    let mut y = position.y;

    // if far enough or not a door
    if perp_wall_dist > 0.2 || (col_data[6] == 2) {
        x += position.dir_x * distance;
        y += position.dir_y * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    // since we can't move in both direction, check just y
    let mut raycast_position_x = raycast_position.clone();
    raycast_position_x.dir_y = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_x, _, _, _) = raycast_column(
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
        // None,
        // None,
        None,
        // None,
        true,
        // None,
        // None,
        true,
    );
    if perp_wall_dist_x > 0.2 {
        x += position.dir_x * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    // if we weren't able to move x, check if we can move y
    let mut raycast_position_y = raycast_position.clone();
    raycast_position_y.dir_x = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_y, _, _, _) = raycast_column(
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
        // None,
        // None,
        None,
        // None,
        true,
        // None,
        // None,
        true,
    );
    if perp_wall_dist_y > 0.2 {
        y += position.dir_y * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    serde_wasm_bindgen::to_value(&vec![x, y]).unwrap()
}

/*
 * Copyright 2022 Google Inc. All Rights Reserved.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use hsl::HSL;
use num_complex::Complex64;
use rand::Rng;
use rayon::prelude::*;
use wasm_bindgen::{prelude::*, Clamped};

#[cfg(feature = "parallel")]
pub use wasm_bindgen_rayon::init_thread_pool;

type RGBA = [u8; 4];

struct Generator {
    width: u32,
    height: u32,
    palette: Box<[RGBA]>,
}

impl Generator {
    fn new(width: u32, height: u32, max_iterations: u32) -> Self {
        let mut rng = rand::thread_rng();

        Self {
            width,
            height,
            palette: (0..max_iterations)
                .map(move |_| {
                    let (r, g, b) = HSL {
                        h: rng.gen_range(0.0..360.0),
                        s: 0.5,
                        l: 0.6,
                    }
                    .to_rgb();
                    [r, g, b, 255]
                })
                .collect(),
        }
    }

    #[allow(clippy::many_single_char_names)]
    fn get_color(&self, x: u32, y: u32) -> &RGBA {
        let c = Complex64::new(
            (f64::from(x) - f64::from(self.width) / 2.0) * 4.0 / f64::from(self.width),
            (f64::from(y) - f64::from(self.height) / 2.0) * 4.0 / f64::from(self.height),
        );
        let mut z = Complex64::new(0.0, 0.0);
        let mut i = 0;
        while z.norm_sqr() < 4.0 {
            if i == self.palette.len() {
                return &self.palette[0];
            }
            z = z.powi(2) + c;
            i += 1;
        }
        &self.palette[i]
    }

    fn iter_row_bytes(&self, y: u32) -> impl '_ + Iterator<Item = u8> {
        (0..self.width)
            .flat_map(move |x| self.get_color(x, y))
            .copied()
    }

    fn iter_bytes(&self) -> impl '_ + ParallelIterator<Item = u8> {
        (0..self.height)
            // Note: when built without atomics, into_par_iter() will
            // automatically fall back to single-threaded mode.
            .into_par_iter()
            .flat_map_iter(move |y| self.iter_row_bytes(y))
    }
}

#[wasm_bindgen]
pub fn generate(width: u32, height: u32, max_iterations: u32) -> Clamped<Vec<u8>> {
    Clamped(
        Generator::new(width, height, max_iterations)
            .iter_bytes()
            .collect(),
    )
}
