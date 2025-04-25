#![feature(map_try_insert)]
#![feature(portable_simd)]
use helpers::{
    fixed_mul, get_bits, get_grid_value, has_bit_set, parse_sprite_texture_array, to_fixed,
    to_fixed_large, Position, Sprite, SpritePart, Texture, TranslationResult,
    WasmStripeHashMapArray, FIXED_ONE, FIXED_SHIFT,
};
use js_sys::Math::atan2;
use smallvec::SmallVec;
use wasm_bindgen::prelude::*;

use rayon::prelude::*;

#[cfg(feature = "parallel")]
pub use wasm_bindgen_rayon::init_thread_pool;

mod helpers;
mod line_intersection;
use geo::{Coord, HausdorffDistance, Line};
use line_intersection::LineInterval;
use std::collections::HashSet;
use std::f32::consts::PI;
use std::{collections::HashMap, f32::MAX};
use web_sys::console;
// let js: JsValue = vec![found_sprites_length as f32].into();
// console::log_2(&"Znj?".into(), &js);
use std::ptr::write_bytes;
use std::slice::from_raw_parts;
use std::slice::from_raw_parts_mut;

pub fn raycast_column(
    column: i32,
    position: Position,
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width_resolution: i32,
    height: i32,
    width: i32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
    sprites_map: Option<&HashMap<(i32, i32), Vec<[f32; 5]>>>,
    skip_sprites_and_writes: bool,
    stop_at_window: bool,
) -> (f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[[f32; 9]; 2]>) {
    let mut met_coords: HashMap<(i32, i32), i32> = HashMap::new();
    let mut window_sprites: SmallVec<[[f32; 9]; 2]> = SmallVec::new();

    let default_sprites_map = HashMap::new();
    let sprites_map = sprites_map.unwrap_or_else(|| &default_sprites_map);

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
    let initial_bit_offset = 16;

    while hit == 0 && remaining_range >= 0 {
        let value: u64 = get_grid_value(map_x, map_y, map_width as i32, map_data);

        // if wall bit is set
        if has_bit_set(value, 0) {
            hit_type = 1 as i8;

            let mut coord_delta_dist_x = MAX;
            let mut coord_delta_dist_y = MAX;
            let mut distance = MAX;
            let mut local_width: f32 = 1.0;
            let mut local_offset: f32 = 1.0;

            // we support two lines per coordinate
            for i in 0..3 {
                // get bit width first so we can skip all the rest
                let mut bit_width = 0;
                match i {
                    0 => {
                        bit_width = get_bits(value, initial_bit_offset + 8);
                    }
                    1 => {
                        bit_width = get_bits(value, initial_bit_offset + 24);
                    }
                    2 => {
                        bit_width = get_bits(value, initial_bit_offset + 40);
                    }
                    _ => (),
                };
                // no shenanigans if the thickness is 0, we'll allow width to be 0 for e.g. windows
                if bit_width == 0 {
                    continue;
                }
                let mut bit_offset = 0;
                let mut bit_thickness = 0;
                let mut bit_offset_secondary = 0;
                let mut is_door = false;
                let mut has_set_north_bit = false;
                let mut is_window = false;
                match i {
                    0 => {
                        bit_offset = get_bits(value, initial_bit_offset);
                        bit_thickness = get_bits(value, initial_bit_offset + 4);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 12);
                        is_door = has_bit_set(value, 5);
                        has_set_north_bit = has_bit_set(value, 6);
                        is_window = has_bit_set(value, 8);
                    }
                    1 => {
                        bit_offset = get_bits(value, initial_bit_offset + 16);
                        bit_thickness = get_bits(value, initial_bit_offset + 20);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 28);
                        is_door = has_bit_set(value, 4);
                        has_set_north_bit = has_bit_set(value, 7);
                        is_window = has_bit_set(value, 9);
                    }
                    2 => {
                        bit_offset = get_bits(value, initial_bit_offset + 32);
                        bit_thickness = get_bits(value, initial_bit_offset + 36);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 44);
                        is_door = has_bit_set(value, 4);
                        has_set_north_bit = has_bit_set(value, 2);
                        is_window = false;
                    }
                    _ => (),
                };
                let is_east = !has_set_north_bit;

                let mut local_delta_dist_x = 0.0;
                let mut local_delta_dist_y = 0.0;

                // from east or west side
                // offset is defined from the east or north
                let offset: f32;
                let distance_offset: f32;

                // TODO: this could be integer math if we went to 16?
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
                    new_map_end_x = new_map_start_x;
                    new_map_start_y = map_y as f32 + offset_secondary;
                    new_map_end_y = new_map_start_y + depth;
                } else {
                    new_map_start_y = map_y as f32 + offset;
                    new_map_end_y = new_map_start_y;
                    new_map_start_x = map_x as f32 + offset_secondary;
                    new_map_end_x = new_map_start_x + depth;
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
                    new_map_between_end_x = new_map_between_start_x + thickness;
                    new_map_between_start_y = map_y as f32 + segment_map_adder;
                    new_map_between_end_y = new_map_between_start_y;
                } else {
                    new_map_between_start_y = map_y as f32 + offset1;
                    new_map_between_end_y = new_map_between_start_y + thickness;
                    new_map_between_start_x = map_x as f32 + segment_map_adder;
                    new_map_between_end_x = new_map_between_start_x;
                }

                // the segment of line between the offsets of the wall
                let segment_between = LineInterval::line_segment(Line {
                    start: (new_map_between_start_x, new_map_between_start_y).into(),
                    end: (new_map_between_end_x, new_map_between_end_y).into(),
                });

                // ray between player position and point on the ray direction
                let line = LineInterval::ray(Line {
                    start: (position.x, position.y).into(),
                    end: (position.x + ray_dir_x, position.y + ray_dir_y).into(),
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
                    let amount_to_move_back = 1.0 - (distance_offset);
                    local_delta_dist_x += delta_dist_x * amount_to_move_back;
                    local_delta_dist_y += delta_dist_y * amount_to_move_back;

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
                        // we'll only use this data if we're stopping at a window or it's not a window
                        if !is_window || stop_at_window {
                            distance = local_distance;
                            coord_delta_dist_x = local_delta_dist_x;
                            coord_delta_dist_y = local_delta_dist_y;
                            side = local_side;
                            wall_width = local_width;
                            wall_offset = local_offset;
                            hit = 1;
                        }
                        // has door bit set
                        if is_door {
                            hit_type = 0x2 as i8;
                        } else if is_window {
                            hit_type = 0x3 as i8;

                            // add to visible sprites
                            if !skip_sprites_and_writes {
                                let window_data = [
                                    local_intersection_coord.x,
                                    local_intersection_coord.y,
                                    0.0,
                                    100.0,
                                    7 as f32,
                                    column as f32,
                                    local_side as f32,
                                    local_offset,
                                    local_width,
                                ];

                                window_sprites.push(window_data);
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
            }
        }

        // handle thick wall
        if value == 1 {
            wall_width = 1.0;
            wall_offset = 0.0;
            hit = 1;
        }

        // only add coord if sprites exist in it;
        // TODO: check more smartly
        if !skip_sprites_and_writes && column % 5 == 0 {
            if let Some(_) = sprites_map.get(&(map_x, map_y)) {
                let _ = met_coords.try_insert((map_x, map_y), 0);
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
    // scale the height according to width so they're square!
    let line_height = width as f32 / 2.0 / perp_wall_dist;
    let aspect_ratio = height as f32 / width as f32;

    let middle_y = height as f32 / 2.0
        + position.pitch as f32
        + position.z / (perp_wall_dist * (2.0 * aspect_ratio));
    let draw_start_y = -line_height / 2.0 + middle_y;
    let draw_end_y = line_height / 2.0 + middle_y;

    wall_x -= wall_x.floor();

    // since we'd like texture to match the width if it's a door
    wall_x -= wall_offset;
    wall_x /= wall_width;

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

    let left = column;
    let wall_height = (draw_end_y - draw_start_y) as i32;

    let alpha_i = (FIXED_ONE - to_fixed(global_alpha)) as i32;

    let col_data = [
        tex_x,
        left,
        draw_start_y as i32,
        wall_height,
        alpha_i,
        hit as i32,
        hit_type as i32,
    ];

    (
        perp_wall_dist,
        col_data,
        met_coords.keys().cloned().collect(),
        window_sprites,
    )
}

#[wasm_bindgen]
pub fn draw_walls_raycast(
    ceiling_floor_img: *mut u8,
    wall_texture: *mut u8,
    door_texture: *mut u8,
    zbuffer_array: *mut f32,
    position: JsValue,
    map_array: *mut u64, // 2D array representing the grid map
    map_width: usize,    // Needed to index into 1D map
    width_resolution: i32,
    height_resolution: i32,
    height: i32,
    width: i32,
    width_spacing: f32,
    light_range: f32,
    range: i32,
    wall_texture_width: i32,
    wall_texture_height: i32,
    door_texture_width: i32,
    door_texture_height: i32,
    found_sprites_array: *mut f32,
    all_sprites_array: *mut f32,
    all_sprites_count: usize,
    sprites_map: &mut WasmStripeHashMapArray,
) -> u32 {
    let img_slice = unsafe {
        std::slice::from_raw_parts_mut(
            ceiling_floor_img,
            (width_resolution * height_resolution * 4) as usize,
        )
    };

    let wall_texture_array = unsafe {
        from_raw_parts(
            wall_texture,
            (wall_texture_width * wall_texture_height * 4) as usize,
        )
    };
    let door_texture_array = unsafe {
        from_raw_parts(
            door_texture,
            (door_texture_width * door_texture_height * 4) as usize,
        )
    };

    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };
    let found_sprites = unsafe {
        from_raw_parts_mut(
            found_sprites_array,
            (all_sprites_count + (2 * width_resolution) as usize) * 9,
        )
    };
    let zbuffer = unsafe { from_raw_parts_mut(zbuffer_array, width_resolution as usize) };

    let mut found_sprites_count = 0;

    let data: Vec<(f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[[f32; 9]; 2]>)> = (0
        ..width_resolution)
        .into_par_iter()
        .map(|column| {
            let (perp_wall_dist, col_data, met_coords, window_sprites) = raycast_column(
                column,
                position,
                map_data,
                map_width,
                width_resolution,
                height_resolution,
                width_resolution,
                light_range,
                range,
                wall_texture_width,
                Some(&sprites_map.get_map()),
                false,
                false,
            );

            (perp_wall_dist, col_data, met_coords, window_sprites)
        })
        .collect();

    let uniqued_met_coords: Vec<&(i32, i32)> = data
        .par_iter()
        .flat_map(|(_, _, met_coords, _)| met_coords)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    for (x, y) in uniqued_met_coords {
        let (map_x, map_y) = (*x as i32, *y as i32);

        if let Some(sprite_list) = sprites_map.get_map().get(&(map_x, map_y)) {
            for &sprite in sprite_list {
                let index = (found_sprites_count as usize) * 9; // Convert u32 to usize

                (found_sprites)[index..index + 5].copy_from_slice(&sprite);
                found_sprites_count += 1;
            }
        }
    }

    let all_window_sprites: Vec<&[f32; 9]> = data
        .iter()
        .flat_map(|(_, _, _, window_sprites)| window_sprites)
        .collect();

    // Compute start position and length
    let start_index = found_sprites_count as usize * 9;
    let total_len = all_window_sprites.len() * 9;

    // Get a mutable slice to just the part we want to fill
    let target_slice = &mut found_sprites[start_index..start_index + total_len];

    // SAFELY split into mutable chunks of 9
    let chunks: Vec<&mut [f32]> = target_slice.chunks_mut(9).collect();

    // Zip input and output together and write in parallel
    chunks
        .into_par_iter()
        .zip(all_window_sprites.par_iter())
        .for_each(|(out_chunk, &sprite)| {
            out_chunk.copy_from_slice(sprite);
        });

    // Update count afterward
    found_sprites_count += all_window_sprites.len() as u32;

    zbuffer
        .par_iter_mut()
        .zip(data.par_iter())
        .for_each(|(zb, (perp_wall_dist, _, _, _))| {
            *zb = *perp_wall_dist;
        });

    let door_texture_data = Texture {
        data: door_texture_array,
        width: door_texture_width,
        height: door_texture_height,
    };
    let wall_texture_data = Texture {
        data: wall_texture_array,
        width: wall_texture_width,
        height: wall_texture_height,
    };

    img_slice
        .par_chunks_mut((width_resolution * 4) as usize)
        .enumerate()
        .for_each(|(screen_y, row)| {
            let screen_y = screen_y as i32;

            for (_, col_data, _, _) in data.iter() {
                let [tex_x, left, draw_start_y, wall_height, global_alpha, hit, col_type] =
                    *col_data;

                if hit == 0 || screen_y < draw_start_y || screen_y >= draw_start_y + wall_height {
                    continue;
                }
                let texture = if col_type == 2 {
                    &door_texture_data
                } else {
                    &wall_texture_data
                };

                let dy = screen_y - draw_start_y;
                let tex_y = dy * texture.height / wall_height;
                let tex_idx = ((tex_y * wall_texture_width + tex_x) * 4) as usize;

                let texel = &texture.data[tex_idx..tex_idx + 3];
                let r = ((texel[0] as i32 * global_alpha) >> FIXED_SHIFT) as u8;
                let g = ((texel[1] as i32 * global_alpha) >> FIXED_SHIFT) as u8;
                let b = ((texel[2] as i32 * global_alpha) >> FIXED_SHIFT) as u8;

                // use std::simd::num::SimdInt;
                // use std::simd::num::SimdUint;
                // use std::simd::Simd;

                // let texel = Simd::<u8, 4>::from_array([
                //     texture.data[tex_idx],
                //     texture.data[tex_idx + 1],
                //     texture.data[tex_idx + 2],
                //     0, // padding to fill 4 lanes
                // ]);
                // // Cast to i32 for fixed-point math
                // let texel_i32 = texel.cast::<i32>();

                // let alpha_vec = Simd::splat(global_alpha);
                // let shifted = (texel_i32 * alpha_vec) >> Simd::splat(FIXED_SHIFT as i32);

                // // Convert back to u8
                // let result: Simd<u8, 4> = shifted.cast();

                let idx = (left * 4) as usize;
                row[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
                // row[idx..idx + 4].copy_from_slice(&[result[0], result[1], result[2], 255]);
            }
        });

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
    light_range: i32,
    map_light: i32,
    floor_texture_width: i32,
    floor_texture_height: i32,
    ceiling_texture_width: i32,
    ceiling_texture_height: i32,
    road_texture_width: i32,
    road_texture_height: i32,
    map_array: *mut u64,
    map_width: usize,
) {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let map_data = unsafe { from_raw_parts(map_array, map_width * map_width) };

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

    let ray_dir_x0 = position.dir_x - position.plane_x;
    let ray_dir_y0 = position.dir_y - position.plane_y;
    let ray_dir_x1 = position.dir_x + position.plane_x;
    let ray_dir_y1 = position.dir_y + position.plane_y;
    let ray_dir_x_dist = ray_dir_x1 - ray_dir_x0;
    let ray_dir_y_dist = ray_dir_y1 - ray_dir_y0;

    let half_height = (ceiling_height_resolution / 2) as i32;
    let scale = ceiling_height_resolution as f32 / height as f32;
    let scaled_pitch = (position.pitch as f32 * scale) as i32;
    let scaled_z = (position.z * scale) as i32;

    let height_resolution_ratio =
        ceiling_height_resolution as f32 / ceiling_width_resolution as f32;
    let distance_divider = (2.0 * height_resolution_ratio) * position.plane_y_initial;

    let img_slice = unsafe {
        from_raw_parts_mut(
            ceiling_floor_img,
            ceiling_width_resolution * ceiling_height_resolution * 4,
        )
    };

    let road_texture_data = Texture {
        data: road_texture_array,
        width: road_texture_width,
        height: road_texture_height,
    };
    let ceiling_texture_data = Texture {
        data: ceiling_texture_array,
        width: ceiling_texture_width,
        height: ceiling_texture_height,
    };
    let floor_texture_data = Texture {
        data: floor_texture_array,
        width: floor_texture_width,
        height: floor_texture_height,
    };
    let map_light_fixed = map_light << FIXED_SHIFT;

    img_slice
        .par_chunks_mut(ceiling_width_resolution * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as i32;
            let is_floor = y > half_height + scaled_pitch;

            let p = if is_floor {
                y - half_height - scaled_pitch
            } else {
                half_height - y + scaled_pitch
            };
            let cam_z = if is_floor {
                half_height + scaled_z
            } else {
                half_height - scaled_z
            };

            let row_distance = cam_z as f32 / (p as f32 * distance_divider);

            let row_distance_fixed = to_fixed(row_distance);

            let alpha_fixed =
                (FIXED_ONE - ((row_distance_fixed / light_range) - map_light_fixed)).max(0);
            let alpha = fixed_mul(alpha_fixed, 256);

            // let alpha_f32 = 1.0 - (row_distance / light_range as f32 - map_light as f32);
            // let alpha = (alpha_f32 * 256.0) as u8;

            let floor_step_x =
                to_fixed(row_distance * ray_dir_x_dist / ceiling_width_resolution as f32);
            let floor_step_y =
                to_fixed(row_distance * ray_dir_y_dist / ceiling_width_resolution as f32);

            let base_x = to_fixed(position.x + row_distance * ray_dir_x0);
            let base_y = to_fixed(position.y + row_distance * ray_dir_y0);

            for (x, pixel) in row.chunks_exact_mut(4).enumerate() {
                let step = x as i32;

                let world_x = base_x + fixed_mul(floor_step_x, step << FIXED_SHIFT);
                let world_y = base_y + fixed_mul(floor_step_y, step << FIXED_SHIFT);

                let map_x = world_x >> FIXED_SHIFT;
                let map_y = world_y >> FIXED_SHIFT;

                let value = get_grid_value(map_x, map_y, map_width as i32, map_data);
                let has_ceiling = has_bit_set(value, 1);
                let has_road = has_bit_set(value, 3);

                let tex = if is_floor && has_road {
                    Some(&road_texture_data)
                } else if is_floor && has_ceiling {
                    Some(&floor_texture_data)
                } else if !is_floor && has_ceiling {
                    Some(&ceiling_texture_data)
                } else {
                    None
                };

                if let Some(tex) = tex {
                    let frac_x = (world_x & (FIXED_ONE - 1)) as usize;
                    let frac_y = (world_y & (FIXED_ONE - 1)) as usize;

                    let tx = (tex.width as usize * frac_x) >> FIXED_SHIFT;
                    let ty = (tex.height as usize * frac_y) >> FIXED_SHIFT;

                    let tex_idx = (ty * tex.width as usize + tx) * 4;

                    let r = (tex.data[tex_idx] as u16 * alpha as u16) >> 8;
                    let g = (tex.data[tex_idx + 1] as u16 * alpha as u16) >> 8;
                    let b = (tex.data[tex_idx + 2] as u16 * alpha as u16) >> 8;

                    pixel.copy_from_slice(&[r as u8, g as u8, b as u8, 255]);
                }
            }
        });
}

pub fn translate_coordinate_to_camera(
    position: Position,
    point_x: f32,
    point_y: f32,
    height_multiplier: f32,
    width: i32,
    height: i32,
    aspect_ratio: f32,
    inv_det: f32,
) -> TranslationResult {
    let half_height = height / 2;
    let half_width = width / 2;

    // translate x, y position to relative to camera
    let sprite_x = point_x - position.x;
    let sprite_y = point_y - position.y;

    // inverse camera matrix calculation
    let transform_x = inv_det * (position.dir_y * sprite_x - position.dir_x * sprite_y)
        / position.plane_y_initial;
    let transform_y = (inv_det * (-position.plane_y * sprite_x + position.plane_x * sprite_y))
        / position.plane_y_initial;

    let screen_x = ((half_width as f32) * (1.0 + (transform_x / transform_y))) as i32;

    // to control the pitch/jump
    let v_move_screen =
        (position.pitch as f32 + (position.z) / (transform_y * (aspect_ratio * 2.0))) as i32;

    // divide by focal length (length of the plane vector)
    let y_height_before_adjustment = (half_width as f32 / (transform_y)) as i32;
    let y_height = (y_height_before_adjustment as f32 * height_multiplier) as i32;
    let height_distance = y_height_before_adjustment - y_height;
    let screen_ceiling_y = half_height - y_height / 2;

    let sprite_ceiling_screen_y = screen_ceiling_y + v_move_screen + height_distance / 2;

    TranslationResult {
        screen_x,
        screen_y_ceiling: sprite_ceiling_screen_y.min(height),
        distance: transform_y,
        full_height: y_height,
    }
}

#[wasm_bindgen]
pub fn draw_sprites_wasm(
    position_js: JsValue,
    ceiling_floor_img: *mut u8,
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
    width_resolution: u32,
    height_resolution: usize,
    found_sprites_count: u32,
    window_texture: *mut u8,
    window_texture_width: i32,
    window_texture_height: i32,
    tree_texture: *mut u8,
    tree_texture_width: i32,
    tree_texture_height: i32,
) -> usize {
    let window_texture_array = unsafe {
        from_raw_parts(
            window_texture,
            (window_texture_width * window_texture_height * 4) as usize,
        )
    };
    let tree_texture_array = unsafe {
        from_raw_parts(
            tree_texture,
            (tree_texture_width * tree_texture_height * 4) as usize,
        )
    };
    let img_slice = unsafe {
        std::slice::from_raw_parts_mut(ceiling_floor_img, width as usize * height as usize * 4)
    };

    let found_sprites_length = found_sprites_count as usize;

    let position: Position = serde_wasm_bindgen::from_value(position_js).unwrap();
    let zbuffer = unsafe { from_raw_parts(zbuffer_array, width_resolution as usize) };
    let sprite_data = unsafe { from_raw_parts(visible_sprites_array, found_sprites_length * 9) };
    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    // copy them since we need to sort them anyway
    let mut sprites: Vec<Sprite> = sprite_data
        .par_chunks(9)
        .map(|chunk| Sprite {
            x: chunk[0],
            y: chunk[1],
            angle: chunk[2] as i32,
            height: chunk[3] as i32,
            r#type: chunk[4] as i32,
            column: chunk[5] as u32,
            side: chunk[6] as u8,
            offset: chunk[7],
            width: chunk[8],
            x_fixed: to_fixed_large(chunk[0]),
            y_fixed: to_fixed_large(chunk[1]),
        })
        .collect();

    let px = to_fixed_large(position.x);
    let py = to_fixed_large(position.y);

    // since we should draw those in the distance first, we sort them
    sprites.sort_unstable_by(|a, b| {
        let da = (px - a.x_fixed).pow(2) + (py - a.y_fixed).pow(2);
        let db = (px - b.x_fixed).pow(2) + (py - b.y_fixed).pow(2);

        db.cmp(&da) // sort descending (farther first)
    });

    // for usage in translate_coordinate_to_camera
    let aspect_ratio = height as f32 / width as f32;
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();

    let sprite_parts_collected: Vec<Option<SpritePart>> = sprites
        .into_par_iter()
        .map(|sprite| {
            let projection = translate_coordinate_to_camera(
                position,
                sprite.x,
                sprite.y,
                sprite.height as f32 / 100.0,
                width,
                height,
                aspect_ratio,
                inv_det,
            );

            let alpha = projection.distance / light_range - map_light;

            // ensure sprites are always at least a little bit visible - alpha 1 is all black
            let alpha_i = (FIXED_ONE - to_fixed(alpha)).clamp(FIXED_ONE / 8, FIXED_ONE) as i32;

            // TODO: this is causing the first one to disappear??
            let (texture_height, texture_width) =
                texture_array.get(&sprite.r#type).copied().unwrap();
            // TODO: remove
            let (texture_height, texture_width) = (tree_texture_height, tree_texture_width);

            if sprite.r#type == 7 {
                // TODO: remove
                let (texture_height, texture_width) = (window_texture_height, window_texture_width);

                // we'll only run into this when we have a window and a wall in the same coord, but we need to check nevertheless
                if projection.distance > zbuffer[sprite.column as usize] {
                    return None;
                }
                // switch which side we were raycasting from to take the fract part to know where the texture was hit
                let mut fract: f32;
                // TODO: maybe not keep all of this in memory and just pass the fract around?
                if sprite.side == 1 {
                    fract = sprite.x.fract();
                } else {
                    fract = sprite.y.fract();
                }
                // since we'd like the texture to match the width
                fract -= sprite.offset;
                fract /= sprite.width;

                let texture_x: i32 = (fract * texture_width as f32) as i32;
                return Some(SpritePart {
                    sprite_type: sprite.r#type,
                    sprite_left_x: (sprite.column),
                    width: 1,
                    screen_y_ceiling: projection.screen_y_ceiling,
                    height: projection.full_height,
                    tex_x1: texture_x,
                    tex_width: 1,
                    alpha: alpha_i,
                    angle: 0,
                });
            }

            if projection.distance < 0.0 {
                return None;
            }
            let texture_aspect_ratio = texture_width as f32 / texture_height as f32;

            let dx = position.x - sprite.x;
            let dy = position.y - sprite.y;
            let angle = atan2(dx as f64, dy as f64);

            // will return from -180 to 180
            let angle_i = (((angle).to_degrees() as i32) + 180 + sprite.angle) % 360;

            let sprite_width =
                (projection.full_height as f32 * texture_aspect_ratio as f32).abs() as i32;

            let mut draw_start_x = (-sprite_width / 2 + projection.screen_x).max(0);
            let mut draw_end_x = (sprite_width / 2 + projection.screen_x).min(width - 1);

            // advance the non-visible parts
            let mut idx_start = draw_start_x;
            while idx_start < width_resolution as i32
                && (projection.distance >= zbuffer[idx_start as usize])
                && draw_start_x + 1 < draw_end_x
            {
                draw_start_x += 1;
                idx_start += 1;
            }

            let mut idx_end = draw_end_x;
            while idx_end >= 0
                && (projection.distance >= zbuffer[idx_end as usize])
                && draw_end_x - 1 > draw_start_x
            {
                draw_end_x -= 1;
                idx_end -= 1;
            }

            let to_remove_texture = projection.screen_x as f32 - (sprite_width as f32 / 2.0);
            let tex_x1 = (((draw_start_x as f32 - to_remove_texture) * texture_width as f32)
                / sprite_width as f32) as i32;
            let tex_x2 = (((draw_end_x as f32 - to_remove_texture) * texture_width as f32)
                / sprite_width as f32) as i32;
            let tex_width = tex_x2 - tex_x1;

            Some(SpritePart {
                sprite_type: sprite.r#type,
                sprite_left_x: draw_start_x as u32,
                width: draw_end_x - draw_start_x,
                screen_y_ceiling: projection.screen_y_ceiling as i32,
                height: (projection.full_height) as i32,
                tex_x1,
                tex_width,
                alpha: alpha_i,
                angle: angle_i,
            })
        })
        .collect();

    let sprite_parts_flattened: Vec<SpritePart> =
        sprite_parts_collected.into_iter().flatten().collect();

    let tree_texture_data = Texture {
        data: tree_texture_array,
        width: tree_texture_width,
        height: tree_texture_height,
    };
    let window_texture_data = Texture {
        data: window_texture_array,
        width: window_texture_width,
        height: window_texture_height,
    };

    img_slice
        .par_chunks_mut(4 * width as usize) // One row at a time
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as i32;
            for sprite in sprite_parts_flattened.iter() {
                if y < sprite.screen_y_ceiling || y >= sprite.screen_y_ceiling + sprite.height {
                    continue;
                }
                let dy = y - sprite.screen_y_ceiling;

                let mut texture_data = &tree_texture_data;
                if sprite.sprite_type == 7 {
                    texture_data = &window_texture_data;
                }

                for dx in 0..sprite.width {
                    let x = sprite.sprite_left_x as i32 + dx;
                    if x < 0 || x >= width {
                        continue;
                    }
                    let idx = (x * 4) as usize;
                    if idx >= row.len() {
                        continue;
                    }

                    let tex_y = dy * texture_data.height / sprite.height;
                    let tex_x = sprite.tex_x1 + dx * sprite.tex_width / sprite.width;
                    let tex_idx = ((tex_y * texture_data.width + tex_x) * 4) as usize;

                    if tex_idx >= texture_data.data.len() {
                        continue;
                    }

                    let texel = &texture_data.data[tex_idx..tex_idx + 4];

                    let a = texel[3] as u16;
                    if a == 0 {
                        continue;
                    }
                    let mut r = ((texel[0] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;
                    let mut g = ((texel[1] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;
                    let mut b = ((texel[2] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;

                    // alpha blending
                    if a != 255 {
                        let current_texel = &row[idx..idx + 4];

                        r = (((a * r as u16) + (current_texel[0] as u16 * (255 - a))) >> 8) as u8;
                        g = (((a * g as u16) + (current_texel[1] as u16 * (255 - a))) >> 8) as u8;
                        b = (((a * b as u16) + (current_texel[2] as u16 * (255 - a))) >> 8) as u8;
                    }

                    row[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        });

    0
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
        light_range,
        range,
        wall_texture_width,
        None,
        true,
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
        light_range,
        range,
        wall_texture_width,
        None,
        true,
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
        light_range,
        range,
        wall_texture_width,
        None,
        true,
        true,
    );
    if perp_wall_dist_y > 0.2 {
        y += position.dir_y * distance;

        return serde_wasm_bindgen::to_value(&vec![x, y]).unwrap();
    }

    serde_wasm_bindgen::to_value(&vec![x, y]).unwrap()
}

#[wasm_bindgen]
pub fn draw_background_image1(
    bg_img_texture: *mut u8,
    ceiling_floor_img: *mut u8,
    texture_width: i32,
    texture_height: i32,
    width: i32,
    height: i32,
    position: JsValue,
    ambient_light: i32,
) {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let direction = position.dir_x.atan2(position.dir_y) + PI;
    let sky_scale = height as f64 / texture_height as f64;
    let sky_width = (texture_width as f64 * sky_scale * 2.0) as i32;
    let circle = 2.0 * PI;
    let left_offset = ((direction / circle) * (sky_width as f32)) as i32;

    let bg_texture_array = unsafe {
        from_raw_parts(
            bg_img_texture,
            (texture_width * texture_height * 4) as usize,
        )
    };

    let img_slice = unsafe {
        std::slice::from_raw_parts_mut(ceiling_floor_img, width as usize * height as usize * 4)
    };

    img_slice
        .par_chunks_mut(width as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let screen_y_pitch = y as i32 - position.pitch;
            let tex_y = (screen_y_pitch * texture_height / height).clamp(0, texture_height - 1);
            let y_idx = tex_y * texture_width;

            row.par_chunks_mut(4).enumerate().for_each(|(x, pixel)| {
                let mut virtual_x = (x as i32 + left_offset) % sky_width;
                if virtual_x < 0 {
                    virtual_x = virtual_x + sky_width;
                }

                let tex_x = virtual_x * texture_width / sky_width;
                let tex_idx_start = ((y_idx + tex_x) * 4) as usize;
                let tex_idx_end = tex_idx_start + 3;

                let tex_data = &bg_texture_array[tex_idx_start..tex_idx_end];
                // no need to copy alpha channel we aren't using any transparency
                pixel[0..3].copy_from_slice(tex_data);
            });
        });
}

#[wasm_bindgen]
pub fn draw_background_image(
    bg_img_texture: *mut u8,
    ceiling_floor_img: *mut u8,
    texture_width: i32,
    texture_height: i32,
    width: i32,
    height: i32,
    position: JsValue,
    ambient_light: i32,
) {
    let position: Position = serde_wasm_bindgen::from_value(position).unwrap();
    let direction = position.dir_x.atan2(position.dir_y) + PI;
    let sky_scale = height as f64 / texture_height as f64;
    let sky_width = (texture_width as f64 * sky_scale * 2.0) as i32;
    let circle = 2.0 * PI;
    let left_offset = ((direction / circle) * (sky_width as f32)) as i32;

    let bg_texture_array = unsafe {
        from_raw_parts(
            bg_img_texture,
            (texture_width * texture_height * 4) as usize,
        )
    };

    let img_slice = unsafe {
        std::slice::from_raw_parts_mut(ceiling_floor_img, width as usize * height as usize * 4)
    };

    img_slice
        .par_chunks_mut(width as usize * 4)
        .enumerate()
        .for_each(|(y, row)| {
            let screen_y_pitch = y as i32 - position.pitch;
            let tex_y = (screen_y_pitch * texture_height / height).clamp(0, texture_height - 1);
            let y_idx = tex_y * texture_width;

            let row_pixel_count = width as usize;
            let sky_w = sky_width as usize;
            let tex_w = texture_width as usize;

            // Determine the starting texture x based on direction offset
            let mut start_tex_x = left_offset % sky_width;
            if start_tex_x < 0 {
                start_tex_x += sky_width;
            }

            // Amount of texture to read
            let read_pixels = row_pixel_count;

            // Number of pixels to read from first segment
            let first_read = (sky_w - start_tex_x as usize).min(read_pixels);
            let second_read = read_pixels - first_read;

            // Map virtual_x to tex_x
            let map_tex_x = |virtual_x: usize| -> usize { virtual_x * tex_w / sky_w };

            // First slice
            for i in 0..first_read {
                let tex_x = map_tex_x(start_tex_x as usize + i);
                let tex_idx = ((y_idx + tex_x as i32) * 4) as usize;
                let out_idx = i * 4;
                row[out_idx..out_idx + 3].copy_from_slice(&bg_texture_array[tex_idx..tex_idx + 3]);
            }

            // Second slice (wrapped)
            for i in 0..second_read {
                let tex_x = map_tex_x(i);
                let tex_idx = ((y_idx + tex_x as i32) * 4) as usize;
                let out_idx = (first_read + i) * 4;
                row[out_idx..out_idx + 3].copy_from_slice(&bg_texture_array[tex_idx..tex_idx + 3]);
            }
        });
}
