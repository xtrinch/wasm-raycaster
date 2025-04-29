#![feature(map_try_insert)]
#![feature(portable_simd)]
use helpers::{
    fixed_mul, get_bits, get_grid_value, has_bit_set, parse_sprite_texture_array, to_fixed,
    to_fixed_large, BackgroundImageWasm, Position, Sprite, SpritePart, Texture, TranslationResult,
    WasmStripeHashMapArray, WasmStripeTextureHashMapArray, FIXED_ONE, FIXED_SHIFT,
};
use js_sys::Float32Array;
use js_sys::Math::atan2;
use smallvec::SmallVec;
use wasm_bindgen::prelude::*;

use rayon::prelude::*;

#[cfg(feature = "parallel")]
pub use wasm_bindgen_rayon::init_thread_pool;

mod helpers;
mod line_intersection;
use geo::{Coord, Distance, Euclidean, Line};
use line_intersection::LineInterval;
use std::collections::HashSet;
use std::f32::consts::PI;
use std::{collections::HashMap, f32::MAX};
use web_sys::console;
// let js: JsValue = vec![found_sprites_length as f32].into();
// console::log_2(&"Znj?".into(), &js);
use std::slice::from_raw_parts;
use std::slice::from_raw_parts_mut;

// TODO: pre-initialize most of this data except position
#[wasm_bindgen]
pub fn render(
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    plane_x: f32,
    plane_y: f32,
    pitch: i32,
    z: i32,
    plane_y_initial: f32,
    render_img: *mut u8,
    zbuffer_array: *mut f32,
    map_array: *mut u64, // 2D array representing the grid map
    map_width: usize,    // Needed to index into 1D map
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    map_light: i32,
    background: &BackgroundImageWasm,
    sprites_map: &WasmStripeHashMapArray,
    sprites_texture_map: &WasmStripeTextureHashMapArray,
    visible_sprites_array: *mut f32,
    all_sprites_count: usize,
    sprites_texture_array: *mut i32,
    sprites_texture_array_length: usize,
) {
    let position = Position {
        x,
        y,
        dir_x,
        dir_y,
        plane_x,
        plane_y,
        pitch,
        z,
        plane_y_initial,
    };

    let img_slice =
        unsafe { std::slice::from_raw_parts_mut(render_img, width as usize * height as usize * 4) };

    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };

    let zbuffer = unsafe { from_raw_parts_mut(zbuffer_array, width as usize) };

    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    let (wall_texture_width, wall_texture_height, _) = *texture_array.get(&1).unwrap();
    let wall_texture = sprites_texture_map.get_map().get(&(1, 0)).unwrap();

    let (ceiling_texture_width, ceiling_texture_height, _) = *texture_array.get(&2).unwrap();
    let ceiling_texture = sprites_texture_map.get_map().get(&(2, 0)).unwrap();

    let (floor_texture_width, floor_texture_height, _) = *texture_array.get(&3).unwrap();
    let floor_texture = sprites_texture_map.get_map().get(&(3, 0)).unwrap();

    let (road_texture_width, road_texture_height, _) = *texture_array.get(&4).unwrap();
    let road_texture = sprites_texture_map.get_map().get(&(4, 0)).unwrap();

    let (door_texture_width, door_texture_height, _) = *texture_array.get(&5).unwrap();
    let door_texture = sprites_texture_map.get_map().get(&(5, 0)).unwrap();

    draw_background_image_prescaled(&position, background, img_slice, width, height);
    draw_ceiling_floor_raycast(
        &position,
        img_slice,
        floor_texture,
        ceiling_texture,
        road_texture,
        width,
        height,
        light_range,
        map_light,
        floor_texture_width,
        floor_texture_height,
        ceiling_texture_width,
        ceiling_texture_height,
        road_texture_width,
        road_texture_height,
        map_data,
        map_width,
    );
    let found_sprites_count = draw_walls_raycast(
        &position,
        img_slice,
        wall_texture,
        door_texture,
        zbuffer,
        map_data,
        map_width,
        width,
        height,
        light_range,
        range,
        wall_texture_width,
        wall_texture_height,
        door_texture_width,
        door_texture_height,
        visible_sprites_array,
        all_sprites_count,
        sprites_map,
    );
    draw_sprites_wasm(
        &position,
        img_slice,
        width,
        height,
        visible_sprites_array,
        zbuffer,
        sprites_texture_array,
        sprites_texture_array_length,
        light_range,
        map_light,
        found_sprites_count,
        sprites_texture_map,
    );
}

pub fn raycast_column(
    column: i32,
    position: &Position,
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    wall_texture_width: i32,
    sprites_map: Option<&HashMap<(i32, i32), Vec<[f32; 5]>>>,
    skip_sprites_and_writes: bool,
    stop_at_window: bool,
) -> (f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[[f32; 10]; 2]>) {
    let mut met_coords: HashMap<(i32, i32), i32> = HashMap::new();
    let mut window_sprites: SmallVec<[[f32; 10]; 2]> = SmallVec::new();

    let default_sprites_map = HashMap::new();
    let sprites_map = sprites_map.unwrap_or_else(|| &default_sprites_map);

    // x-coordinate in camera space
    let camera_x = (2.0 * (column as f32) / (width as f32)) - 1.0;

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

    let mut hit = false;
    let mut hit_type: i8 = 1;
    let mut remaining_range: i8 = range;
    let mut wall_width = 1.0;
    let mut wall_offset = 0.0;
    let initial_bit_offset = 16;

    let aspect_ratio = height as f32 / width as f32;
    let position_coord = Coord::from([position.x, position.y]);

    while !hit && remaining_range >= 0 {
        let value: u64 = get_grid_value(map_x, map_y, map_width as i32, map_data);
        let num_walls = get_bits(value, 12); // since the upper two are reserves we can afford this

        // if wall bit is set
        if num_walls > 0 {
            hit_type = 1 as i8;

            let mut coord_delta_dist_x = MAX;
            let mut coord_delta_dist_y = MAX;
            let mut distance = MAX;

            // we support two lines per coordinate
            for i in 0..num_walls {
                // get bit width first so we can skip all the rest
                let mut bit_width = 0;
                let mut local_width: f32 = 1.0;
                let mut local_offset: f32 = 1.0;
                let mut bit_offset = 0;
                let mut bit_thickness = 0;
                let mut bit_offset_secondary = 0;
                let mut is_door = false;
                let mut is_window = false;
                let mut is_east = false;

                match i {
                    0 => {
                        bit_width = get_bits(value, initial_bit_offset + 8);
                        bit_offset = get_bits(value, initial_bit_offset);
                        bit_thickness = get_bits(value, initial_bit_offset + 4);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 12);
                        is_door = has_bit_set(value, 5);
                        is_east = !has_bit_set(value, 6);
                        is_window = has_bit_set(value, 8);
                    }
                    1 => {
                        bit_width = get_bits(value, initial_bit_offset + 24);
                        bit_offset = get_bits(value, initial_bit_offset + 16);
                        bit_thickness = get_bits(value, initial_bit_offset + 20);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 28);
                        is_door = has_bit_set(value, 4);
                        is_east = !has_bit_set(value, 7);
                        is_window = has_bit_set(value, 9);
                    }
                    2 => {
                        bit_width = get_bits(value, initial_bit_offset + 40);
                        bit_offset = get_bits(value, initial_bit_offset + 32);
                        bit_thickness = get_bits(value, initial_bit_offset + 36);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 44);
                        is_door = has_bit_set(value, 4);
                        is_east = !has_bit_set(value, 2);
                        is_window = false;
                    }
                    _ => (),
                };

                let mut local_delta_dist_x = 0.0;
                let mut local_delta_dist_y = 0.0;

                // from east or west side
                // offset is defined from the east or north
                let offset: f32;
                let distance_offset: f32;

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
                    start: (new_map_start_x, new_map_start_y).into(),
                    end: (new_map_end_x, new_map_end_y).into(),
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

                // ray between player position and point on the ray direction
                let line = LineInterval::ray(Line {
                    start: (position.x, position.y).into(),
                    end: (position.x + ray_dir_x, position.y + ray_dir_y).into(),
                });

                // check main segment line
                let intersection = segment.relate(&line).unique_intersection();

                let mut local_hit = false;
                let mut local_side = 0;
                let mut local_intersection_coord: Coord<f32> = Coord::zero();
                if let Some(coord) = intersection {
                    local_intersection_coord = coord;
                    local_hit = true;

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
                    // the segment of line between the offsets of the wall
                    let segment_between = LineInterval::line_segment(Line {
                        start: (new_map_between_start_x, new_map_between_start_y).into(),
                        end: (new_map_between_end_x, new_map_between_end_y).into(),
                    });

                    // check line between segments of thickness
                    let intersection_between = segment_between.relate(&line).unique_intersection();
                    if let Some(coord) = intersection_between {
                        local_intersection_coord = coord;
                        local_hit = true;
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
                if local_hit {
                    // take the shortest of the two paths
                    let local_distance =
                        Euclidean.distance(local_intersection_coord, position_coord);
                    if local_distance < distance {
                        // we'll only use this data if we're stopping at a window or it's not a window
                        if !is_window || stop_at_window {
                            distance = local_distance;
                            coord_delta_dist_x = local_delta_dist_x;
                            coord_delta_dist_y = local_delta_dist_y;
                            side = local_side;
                            wall_width = local_width;
                            wall_offset = local_offset;
                            hit = true;
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
                                    12 as f32, // texture type
                                    column as f32,
                                    local_side as f32,
                                    local_offset,
                                    local_width,
                                    local_distance,
                                ];

                                window_sprites.push(window_data);
                            }
                        } else {
                            hit_type = 1;
                        }
                    }
                }
            }
            if hit {
                side_dist_x += coord_delta_dist_x;
                side_dist_y += coord_delta_dist_y;
            }
        }

        // handle thick wall
        if value == 1 {
            wall_width = 1.0;
            wall_offset = 0.0;
            hit = true;
        }

        // only add coord if sprites exist in it;
        // TODO: check more smartly
        if !skip_sprites_and_writes && column % 5 == 0 {
            if let Some(_) = sprites_map.get(&(map_x, map_y)) {
                let _ = met_coords.try_insert((map_x, map_y), 0);
            }
        }

        // don't do any more coordinate increments if hit
        if hit {
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
    let line_height = (width as f32 / 2.0 / perp_wall_dist) as i32;

    let middle_y = height / 2
        + position.pitch
        + (position.z as f32 / (perp_wall_dist * (2.0 * aspect_ratio))) as i32;
    let draw_start_y = -line_height / 2 + middle_y;
    let draw_end_y = line_height / 2 + middle_y;

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
    let mut global_alpha = perp_wall_dist / light_range as f32;
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

    let wall_height = draw_end_y - draw_start_y;

    let alpha_i = (FIXED_ONE - to_fixed(global_alpha)) as i32;

    let col_data = [
        tex_x,
        column,
        draw_start_y,
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

pub fn draw_walls_raycast(
    position: &Position,
    img_slice: &mut [u8],
    wall_texture_array: &Vec<u8>,
    door_texture_array: &Vec<u8>,
    zbuffer: &mut [f32],
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    wall_texture_width: i32,
    wall_texture_height: i32,
    door_texture_width: i32,
    door_texture_height: i32,
    found_sprites_array: *mut f32,
    all_sprites_count: usize,
    sprites_map: &WasmStripeHashMapArray,
) -> u32 {
    let found_sprites = unsafe {
        from_raw_parts_mut(
            found_sprites_array,
            (all_sprites_count + (2 * width) as usize) * 10,
        )
    };

    let mut found_sprites_count = 0;

    let data: Vec<(f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[[f32; 10]; 2]>)> = (0..width)
        .into_par_iter()
        .map(|column| {
            let (perp_wall_dist, col_data, met_coords, window_sprites) = raycast_column(
                column,
                position,
                map_data,
                map_width,
                width,
                height,
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
        .iter()
        .flat_map(|(_, _, met_coords, _)| met_coords)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    for (x, y) in uniqued_met_coords {
        let (map_x, map_y) = (*x as i32, *y as i32);

        if let Some(sprite_list) = sprites_map.get_map().get(&(map_x, map_y)) {
            for &sprite in sprite_list {
                let index = (found_sprites_count as usize) * 10; // Convert u32 to usize

                (found_sprites)[index..index + 5].copy_from_slice(&sprite);
                found_sprites_count += 1;
            }
        }
    }

    let all_window_sprites: Vec<&[f32; 10]> = data
        .iter()
        .flat_map(|(_, _, _, window_sprites)| window_sprites)
        .collect();

    // Compute start position and length
    let start_index = found_sprites_count as usize * 10;
    let total_len = all_window_sprites.len() * 10;

    // Get a mutable slice to just the part we want to fill
    let target_slice = &mut found_sprites[start_index..start_index + total_len];

    // SAFELY split into mutable chunks of 9
    let mut chunks: Vec<&mut [f32]> = target_slice.chunks_mut(10).collect();

    // Zip input and output together and write in parallel
    chunks
        .iter_mut()
        .zip(all_window_sprites.iter())
        .for_each(|(out_chunk, &sprite)| {
            out_chunk.copy_from_slice(sprite);
        });

    // Update count afterward
    found_sprites_count += all_window_sprites.len() as u32;

    zbuffer
        .iter_mut()
        .zip(data.iter())
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

    let col_datas: Vec<&[i32; 7]> = data
        .par_iter()
        .map(|(_, col_data, _, _)| col_data)
        .collect();

    img_slice
        .par_chunks_mut((width * 4) as usize)
        .enumerate()
        .for_each(|(screen_y, row)| {
            let screen_y = screen_y as i32;

            for col_data in col_datas.iter() {
                let [tex_x, left, draw_start_y, wall_height, global_alpha, hit, col_type] =
                    **col_data;

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

                let idx = (left * 4) as usize;
                row[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
                // row[idx..idx + 4].copy_from_slice(&[result[0], result[1], result[2], 255]);
            }
        });

    found_sprites_count
}

pub fn draw_ceiling_floor_raycast(
    position: &Position,
    img_slice: &mut [u8],
    floor_texture_array: &Vec<u8>,
    ceiling_texture_array: &Vec<u8>,
    road_texture_array: &Vec<u8>,
    width: i32,
    height: i32,
    light_range: i32,
    map_light: i32,
    floor_texture_width: i32,
    floor_texture_height: i32,
    ceiling_texture_width: i32,
    ceiling_texture_height: i32,
    road_texture_width: i32,
    road_texture_height: i32,
    map_data: &[u64],
    map_width: usize,
) {
    let ray_dir_x0 = position.dir_x - position.plane_x;
    let ray_dir_y0 = position.dir_y - position.plane_y;
    let ray_dir_x1 = position.dir_x + position.plane_x;
    let ray_dir_y1 = position.dir_y + position.plane_y;
    let ray_dir_x_dist = ray_dir_x1 - ray_dir_x0;
    let ray_dir_y_dist = ray_dir_y1 - ray_dir_y0;

    let half_height = (height / 2) as i32;
    let floor_cam_z = half_height + position.z as i32;
    let ceiling_cam_z = half_height - position.z as i32;
    let middle_view_y = half_height + position.pitch;

    // if we're above the ceiling
    let mut is_above_ceiling = false;
    if ceiling_cam_z < 0 {
        is_above_ceiling = true;
        // move the floor up so it appears as a ceiling from the top
        // floor_cam_z -= height as i32;
    }

    let height_ratio = height as f32 / width as f32;
    let distance_divider = (2.0 * height_ratio) * position.plane_y_initial;

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
        .par_chunks_mut((width * 4) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as i32;

            // if we're drawing the bottom half of the screen
            let is_floor = y > middle_view_y;

            if !is_floor && is_above_ceiling {
                return;
            }
            let p = if is_floor {
                -(middle_view_y) + y
            } else {
                middle_view_y - y
            };
            let cam_z = if is_floor { floor_cam_z } else { ceiling_cam_z };

            let row_distance = cam_z as f32 / (p as f32 * distance_divider);
            let row_distance_fixed = to_fixed(row_distance);

            let alpha_fixed =
                (FIXED_ONE - ((row_distance_fixed / light_range) - map_light_fixed)).max(0);
            let alpha = fixed_mul(alpha_fixed, 256);

            // let alpha_f32 = 1.0 - (row_distance / light_range as f32 - map_light as f32);
            // let alpha = (alpha_f32 * 256.0) as u8;

            let floor_step_x = to_fixed(row_distance * ray_dir_x_dist / width as f32);
            let floor_step_y = to_fixed(row_distance * ray_dir_y_dist / width as f32);

            let base_x = to_fixed(position.x + row_distance * ray_dir_x0);
            let base_y = to_fixed(position.y + row_distance * ray_dir_y0);

            row.chunks_exact_mut(4).enumerate().for_each(|(x, pixel)| {
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
                    let texel = &tex.data[tex_idx..tex_idx + 3];
                    let r = (texel[0] as u16 * alpha as u16) >> 8;
                    let g = (texel[1] as u16 * alpha as u16) >> 8;
                    let b = (texel[2] as u16 * alpha as u16) >> 8;

                    pixel.copy_from_slice(&[r as u8, g as u8, b as u8, 255]);
                }
            });
        });
}

pub fn translate_coordinate_to_camera(
    position: &Position,
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
        (position.pitch as f32 + (position.z as f32) / (transform_y * (aspect_ratio * 2.0))) as i32;

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

pub fn draw_sprites_wasm(
    position: &Position,
    img_slice: &mut [u8],
    width: i32,
    height: i32,
    visible_sprites_array: *mut f32,
    zbuffer: &mut [f32],
    sprites_texture_array: *const i32,
    sprites_texture_array_length: usize,
    light_range: i32,
    map_light: i32,
    found_sprites_count: u32,
    sprites_texture_map: &WasmStripeTextureHashMapArray,
) {
    let sprite_data =
        unsafe { from_raw_parts(visible_sprites_array, found_sprites_count as usize * 10) };
    let texture_array =
        parse_sprite_texture_array(sprites_texture_array, sprites_texture_array_length);

    let px = to_fixed_large(position.x);
    let py = to_fixed_large(position.y);

    // copy them since we need to sort them anyway
    let mut sprites: Vec<Sprite> = sprite_data
        .par_chunks(10)
        .map(|chunk| {
            let sprite_type = chunk[4] as i32;
            let x = chunk[0];
            let x_fixed = to_fixed_large(x);
            let y = chunk[1];
            let y_fixed = to_fixed_large(y);
            let distance_fixed = (px - x_fixed).pow(2) + (py - y_fixed).pow(2);
            Sprite {
                x,
                y,
                angle: chunk[2] as i32,
                height: chunk[3] as i32,
                r#type: sprite_type,
                column: chunk[5] as u32,
                side: chunk[6] as u8,
                offset: chunk[7],
                width: chunk[8],
                distance: chunk[9],
                distance_fixed,
                x_fixed,
                y_fixed,
            }
        })
        .collect();

    // since we should draw those in the distance first, we sort them
    sprites.sort_unstable_by(|a, b| {
        let da = a.distance_fixed;
        let db = b.distance_fixed;

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

            let alpha = projection.distance / light_range as f32 - map_light as f32;

            // ensure sprites are always at least a little bit visible - alpha 1 is all black
            let alpha_i = (FIXED_ONE - to_fixed(alpha)).clamp(FIXED_ONE / 8, FIXED_ONE) as i32;

            let dx = position.x - sprite.x;
            let dy = position.y - sprite.y;
            let angle = atan2(dx as f64, dy as f64);

            // will return from -180 to 180
            let angle_i = (((angle).to_degrees() as i32) + 180 + sprite.angle) % 360;

            let mut angle_index = (angle_i) / 45; // Default to 1 if the result is 0

            let (texture_height, texture_width, angles) =
                *texture_array.get(&sprite.r#type).unwrap();

            // if there's no textures for other angles
            if angles <= angle_index {
                angle_index = 0;
            }
            let texture_data = sprites_texture_map
                .get_map()
                .get(&(sprite.r#type, angle_index))
                .unwrap();

            // windows; TODO: to enum
            if sprite.r#type == 12 {
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
                    full_texture_height: texture_height,
                    full_texture_width: texture_width,
                    full_texture_data: texture_data,
                });
            }

            if projection.distance < 0.0 {
                return None;
            }
            let texture_aspect_ratio = texture_width as f32 / texture_height as f32;

            let sprite_width = (projection.full_height as f32 * texture_aspect_ratio as f32) as i32;

            let mut draw_start_x = (-sprite_width / 2 + projection.screen_x).max(0);
            let mut draw_end_x = (sprite_width / 2 + projection.screen_x).min(width - 1);

            // advance the non-visible parts
            let mut idx_start = draw_start_x;
            while idx_start < width as i32
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

            let to_remove_texture = projection.screen_x - (sprite_width / 2);
            let tex_x1 = ((draw_start_x - to_remove_texture) * texture_width) / sprite_width;
            let tex_x2 = ((draw_end_x - to_remove_texture) * texture_width) / sprite_width;
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
                full_texture_height: texture_height,
                full_texture_width: texture_width,
                full_texture_data: texture_data,
            })
        })
        .collect();

    let sprite_parts_flattened: Vec<SpritePart> =
        sprite_parts_collected.into_iter().flatten().collect();

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

                let tex_y = dy * sprite.full_texture_height / sprite.height;
                let y_tex_idx = tex_y * sprite.full_texture_width;
                for dx in 0..sprite.width {
                    let tex_x = sprite.tex_x1 + dx * sprite.tex_width / sprite.width;
                    let tex_idx = ((y_tex_idx + tex_x) * 4) as usize;

                    let texel = &sprite.full_texture_data[tex_idx..tex_idx + 4];

                    let a = texel[3] as u16;
                    if a == 0 {
                        continue;
                    }
                    let mut r = ((texel[0] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;
                    let mut g = ((texel[1] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;
                    let mut b = ((texel[2] as i32 * sprite.alpha) >> FIXED_SHIFT) as u8;

                    let x = sprite.sprite_left_x as i32 + dx;
                    let idx = (x * 4) as usize;

                    // alpha blending
                    if a != 255 {
                        let current_texel = &row[idx..idx + 4];
                        let inverted_alpha = 255 - a;
                        r = (((a * r as u16) + (current_texel[0] as u16 * inverted_alpha)) >> 8)
                            as u8;
                        g = (((a * g as u16) + (current_texel[1] as u16 * inverted_alpha)) >> 8)
                            as u8;
                        b = (((a * b as u16) + (current_texel[2] as u16 * inverted_alpha)) >> 8)
                            as u8;
                    }

                    row[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        });
}

#[wasm_bindgen]
pub fn draw_background_image_prescaled(
    position: &Position,
    background: &BackgroundImageWasm,
    img_slice: &mut [u8],
    width: i32,
    height: i32,
) {
    let direction = position.dir_x.atan2(position.dir_y) + PI;

    let pre_scaled = &background.get_data();
    let sky_width = background.get_width();

    let circle = 2.0 * std::f32::consts::PI;
    let mut left_offset = ((direction / circle) * (sky_width as f32)) as i32;
    if left_offset < 0 {
        left_offset += sky_width;
    }

    let pre_scaled_len = pre_scaled.len();

    img_slice
        .par_chunks_mut((width * 4) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let screen_y_pitch = y as i32 - position.pitch;
            if screen_y_pitch < 0 || screen_y_pitch >= height {
                return;
            }

            let row_start = (screen_y_pitch * sky_width * 4) as usize;

            let sky_w_bytes = (sky_width * 4) as usize;
            let screen_w_bytes = (width * 4) as usize;

            let start = ((left_offset * 4) as usize) % sky_w_bytes;

            if start + screen_w_bytes <= sky_w_bytes {
                let idx_start = row_start + start;
                let idx_end = row_start + start + screen_w_bytes;
                if idx_end < pre_scaled_len {
                    row.copy_from_slice(&pre_scaled[idx_start..idx_end]);
                }
            } else {
                let first_part = sky_w_bytes - start;

                let idx_start1 = row_start + start;
                let idx_end1 = row_start + sky_w_bytes;
                if idx_end1 < pre_scaled_len {
                    row[..first_part].copy_from_slice(&pre_scaled[idx_start1..idx_end1]);
                }

                let idx_start2 = row_start;
                let idx_end2 = row_start + (screen_w_bytes - first_part);
                if idx_end2 < pre_scaled_len {
                    row[first_part..].copy_from_slice(&pre_scaled[idx_start2..idx_end2]);
                }
            }
        });
}

// move if no wall in front of you
#[wasm_bindgen]
pub fn walk(
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    plane_x: f32,
    plane_y: f32,
    pitch: i32,
    z: i32,
    plane_y_initial: f32,
    distance: f32,
    map_array: *mut u64,
    map_width: i32,
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    wall_texture_width: i32,
) -> Float32Array {
    let position = Position {
        x,
        y,
        dir_x,
        dir_y,
        plane_x,
        plane_y,
        pitch,
        z,
        plane_y_initial,
    };

    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };

    let mut raycast_position = position.clone();

    // check behind you by turning
    if distance < 0.0 {
        raycast_position.dir_x = position.dir_x * -1.0;
        raycast_position.dir_y = position.dir_y * -1.0;
    }

    // raycast middle column to get the distance
    let (perp_wall_dist, col_data, _, _) = raycast_column(
        (width / 2) as i32,
        &raycast_position,
        map_data,
        map_width as usize,
        width,
        height,
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

        let result = vec![x, y];
        return Float32Array::from(result.as_slice());
    }

    // since we can't move in both direction, check just y
    let mut raycast_position_x = raycast_position.clone();
    raycast_position_x.dir_y = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_x, _, _, _) = raycast_column(
        (width / 2) as i32,
        &raycast_position_x,
        map_data,
        map_width as usize,
        width,
        height,
        light_range,
        range,
        wall_texture_width,
        None,
        true,
        true,
    );
    if perp_wall_dist_x > 0.2 {
        x += position.dir_x * distance;

        let result = vec![x, y];
        return Float32Array::from(result.as_slice());
    }

    // if we weren't able to move x, check if we can move y
    let mut raycast_position_y = raycast_position.clone();
    raycast_position_y.dir_x = 0.0;

    // raycast middle column to get the distance
    let (perp_wall_dist_y, _, _, _) = raycast_column(
        (width / 2) as i32,
        &raycast_position_y,
        map_data,
        map_width as usize,
        width,
        height,
        light_range,
        range,
        wall_texture_width,
        None,
        true,
        true,
    );
    if perp_wall_dist_y > 0.2 {
        y += position.dir_y * distance;

        let result = vec![x, y];
        return Float32Array::from(result.as_slice());
    }

    let result = vec![x, y];
    Float32Array::from(result.as_slice())
}

#[wasm_bindgen]
pub fn rotate_view(
    frame_time: f32,
    multiplier: i32,
    dir_x: f32,
    dir_y: f32,
    plane_x: f32,
    plane_y: f32,
) -> Float32Array {
    let rot_speed = 4.0 * (PI / 5.0) * frame_time * multiplier as f32;

    let cos_r = rot_speed.cos();
    let sin_r = rot_speed.sin();

    let new_dir_x = dir_x * cos_r - dir_y * sin_r;
    let new_dir_y = dir_x * sin_r + dir_y * cos_r;

    let new_plane_x = plane_x * cos_r - plane_y * sin_r;
    let new_plane_y = plane_x * sin_r + plane_y * cos_r;

    let result = vec![new_dir_x, new_dir_y, new_plane_x, new_plane_y];
    Float32Array::from(result.as_slice())
}
