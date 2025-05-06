#![feature(map_try_insert)]
#![feature(portable_simd)]
use helpers::{
    fixed_mul, get_bits, get_grid_value, has_bit_set, to_fixed, to_fixed_large,
    BackgroundImageWasm, Position, Sprite, SpritePart, Texture, TextureType, TranslationResult,
    WasmStripePerCoordMap, WasmTextureMap, WasmTextureMetaMap, FIXED_ONE, FIXED_SHIFT,
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

#[wasm_bindgen]
#[inline(never)]
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
    sprites_map: &WasmStripePerCoordMap, // sprites per x y coordinate
    sprites_texture_map: &WasmTextureMap, // contains textures along with angled textures
    sprites_texture_meta_map: &WasmTextureMetaMap,
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
        map_x: x.floor() as i32,
        map_y: y.floor() as i32,
    };

    let img_slice =
        unsafe { std::slice::from_raw_parts_mut(render_img, width as usize * height as usize * 4) };

    let map_data = unsafe { from_raw_parts(map_array, (map_width * map_width) as usize) };

    let zbuffer = unsafe { from_raw_parts_mut(zbuffer_array, width as usize) };

    let wall_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::WALL as i32))
        .unwrap();
    let wall_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::WALL as i32, 0))
        .unwrap();

    let ceiling_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::CEILING as i32))
        .unwrap();
    let ceiling_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::CEILING as i32, 0))
        .unwrap();

    let floor_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::FLOOR as i32))
        .unwrap();
    let floor_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::FLOOR as i32, 0))
        .unwrap();

    let road_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::ROAD as i32))
        .unwrap();
    let road_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::ROAD as i32, 0))
        .unwrap();

    let door_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::DOOR as i32))
        .unwrap();
    let door_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::DOOR as i32, 0))
        .unwrap();

    let mut found_sprites: SmallVec<[Sprite; 1024]> = vec![].into();

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
        floor_texture_meta.width,
        floor_texture_meta.height,
        ceiling_texture_meta.width,
        ceiling_texture_meta.height,
        road_texture_meta.width,
        road_texture_meta.height,
        map_data,
        map_width,
    );
    draw_walls_raycast(
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
        wall_texture_meta.width,
        wall_texture_meta.height,
        door_texture_meta.width,
        door_texture_meta.height,
        sprites_map,
        &mut found_sprites,
    );
    draw_sprites_wasm(
        &position,
        img_slice,
        width,
        height,
        zbuffer,
        light_range,
        map_light,
        sprites_texture_map,
        sprites_texture_meta_map,
        &mut found_sprites,
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
    sprites_map: Option<&HashMap<(i32, i32), Vec<Sprite>>>,
    skip_sprites_and_writes: bool,
    stop_at_window: bool,
) -> (f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[Sprite; 2]>) {
    let mut met_coords: Vec<(i32, i32)> = Vec::new();
    let mut window_sprites: SmallVec<[Sprite; 2]> = SmallVec::with_capacity(2);

    let default_sprites_map = HashMap::new();
    let sprites_map = sprites_map.unwrap_or_else(|| &default_sprites_map);

    // x-coordinate in camera space
    let camera_x = (2.0 * (column as f32) / (width as f32)) - 1.0;

    let ray_dir_x = position.dir_x + position.plane_x * camera_x;
    let ray_dir_y = position.dir_y + position.plane_y * camera_x;

    let mut map_x = position.map_x;
    let mut map_y = position.map_y;

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

    // local pre-initialized while/for loop variables
    let mut bit_width = 0;
    let mut local_width: f32 = 1.0;
    let mut local_offset: f32 = 1.0;
    let mut bit_offset = 0;
    let mut bit_thickness = 0;
    let mut bit_offset_secondary = 0;
    let mut is_door = false;
    let mut is_window = false;
    let mut is_east = false;
    let mut local_distance_multiplier = 0.0;
    let mut local_side = 0;
    let mut local_intersection_coord: Coord<f32> = Coord::zero();
    // from east or west side
    // offset is defined from the east or north
    let mut offset: f32;
    let mut distance_offset: f32;
    let mut ray_dirs: [f32; 2];
    let mut sides: [i32; 2];
    let mut new_map_start_x;
    let mut new_map_end_x;
    let mut new_map_start_y;
    let mut new_map_end_y;
    let mut segment_map_adder;

    // ray between player position and point on the ray direction
    let line = LineInterval::ray(Line {
        start: (position.x, position.y).into(),
        end: (position.x + ray_dir_x, position.y + ray_dir_y).into(),
    });

    while !hit && remaining_range >= 0 {
        let value: u64 = get_grid_value(map_x, map_y, map_width as i32, map_data);
        let num_walls = get_bits(value, 12); // since the upper two are reserves we can afford this

        // if wall bit is set
        if num_walls > 0 {
            hit_type = 1 as i8;

            let mut distance_multiplier = 0.0; // how much to move back/forward the distance due to internal offsets
            let mut distance = MAX;

            // we support up to three lines per coordinate
            for i in 0..num_walls {
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

                let offset1: f32 = (bit_offset % 11) as f32 / 10.0;
                let thickness: f32 = (bit_thickness % 11) as f32 / 10.0;
                let offset_secondary: f32 = (bit_offset_secondary % 11) as f32 / 10.0;
                let depth: f32 = (bit_width % 11) as f32 / 10.0;

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

                // check main segment line
                let intersection = segment.relate(&line).unique_intersection();

                let mut local_hit = false;
                if let Some(coord) = intersection {
                    local_intersection_coord = coord;
                    local_hit = true;

                    // move it back for the amount it should move back (assign to both even though only 1 will be used, x for east/west and y for north/south)
                    local_distance_multiplier = 1.0 - (distance_offset);

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
                    if ray_dirs[1] > 0.0 {
                        // depending on which side we're looking at the space between the offsets from
                        segment_map_adder = offset_secondary;
                    } else {
                        segment_map_adder = offset_secondary + depth;
                    }

                    if is_east {
                        new_map_start_x = map_x as f32 + offset1;
                        new_map_end_x = new_map_start_x + thickness;
                        new_map_start_y = map_y as f32 + segment_map_adder;
                        new_map_end_y = new_map_start_y;
                    } else {
                        new_map_start_x = map_x as f32 + segment_map_adder;
                        new_map_end_x = new_map_start_x;
                        new_map_start_y = map_y as f32 + offset1;
                        new_map_end_y = new_map_start_y + thickness;
                    }

                    // the segment of line between the offsets of the wall
                    let segment_between = LineInterval::line_segment(Line {
                        start: (new_map_start_x, new_map_start_y).into(),
                        end: (new_map_end_x, new_map_end_y).into(),
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
                            // && move it forward for the amount it should move forward due to secondary offset
                            // if we're looking at it from the shortened side
                            local_distance_multiplier = 1.0 - depth - offset_secondary;
                        } else {
                            // move it back for the amount it should move back due to secondary offset
                            local_distance_multiplier = offset_secondary;
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
                            side = local_side;
                            wall_width = local_width;
                            wall_offset = local_offset;
                            hit = true;
                            distance_multiplier = local_distance_multiplier;
                        }
                        // has door bit set
                        if is_door {
                            hit_type = 0x2 as i8;
                        } else if is_window {
                            hit_type = 0x3 as i8;

                            // switch which side we were raycasting from to take the fract part to know where the texture was hit
                            let mut fract: f32;
                            if local_side == 1 {
                                fract = local_intersection_coord.x.fract();
                            } else {
                                fract = local_intersection_coord.y.fract();
                            }
                            // since we'd like the texture to match the width
                            fract -= local_offset;
                            fract /= local_width;

                            // add to visible sprites
                            if !skip_sprites_and_writes {
                                window_sprites.push(Sprite {
                                    x: local_intersection_coord.x,
                                    y: local_intersection_coord.y,
                                    angle: 0,
                                    height: 100,
                                    r#type: TextureType::WINDOW as i32,
                                    column: column as u32,
                                    distance: local_distance,
                                    distance_fixed: 0,
                                    dx: 0.,
                                    dy: 0.,
                                    fract,
                                });
                            }
                        } else {
                            hit_type = 1;
                        }
                    }
                }
            }
            if hit {
                side_dist_x += delta_dist_x * distance_multiplier;
                side_dist_y += delta_dist_y * distance_multiplier;
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
                let _ = met_coords.push((map_x, map_y));
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
    let line_height = ((width / 2) as f32 / perp_wall_dist) as i32;

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
    let tex_x = wall_texture_width - tex_x - 1;

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

    // TODO: to struct for readability
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
        met_coords.to_vec(),
        window_sprites,
    )
}

#[inline(never)]
#[no_mangle]
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
    sprites_map: &WasmStripePerCoordMap,
    found_sprites: &mut SmallVec<[Sprite; 1024]>,
) {
    let data: Vec<(f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[Sprite; 2]>)> = (0..width)
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

    let uniqued_met_coords: HashSet<(i32, i32)> = data
        .iter()
        .flat_map(|(_, _, met_coords, _)| met_coords.iter().copied())
        .collect();

    let sprites_map = sprites_map.get_map();

    for (x, y) in &uniqued_met_coords {
        let (map_x, map_y) = (*x as i32, *y as i32);

        if let Some(sprite_list) = sprites_map.get(&(map_x, map_y)) {
            found_sprites.extend((*sprite_list).clone());
        }
    }

    for (idx, (perp_wall_dist, _, _, window_sprites)) in data.iter().enumerate() {
        zbuffer[idx] = *perp_wall_dist;
        found_sprites.extend((*window_sprites).clone());
    }

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
        .par_chunks_mut((width * 4) as usize)
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

                let texel = unsafe { texture.data.get_unchecked(tex_idx..tex_idx + 3) };

                let r = ((texel[0] as i32 * global_alpha) >> FIXED_SHIFT) as u8;
                let g = ((texel[1] as i32 * global_alpha) >> FIXED_SHIFT) as u8;
                let b = ((texel[2] as i32 * global_alpha) >> FIXED_SHIFT) as u8;

                let idx = (left * 4) as usize;
                row[idx..idx + 4].copy_from_slice(&[r, g, b, 255]);
            }
        });
}

#[inline(never)]
#[no_mangle]
pub fn draw_ceiling_floor_raycast(
    position: &Position,
    img_slice: &mut [u8],
    floor_texture_array: &[u8],
    ceiling_texture_array: &[u8],
    road_texture_array: &[u8],
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

            let mut world_x = base_x;
            let mut world_y = base_y;

            row.chunks_exact_mut(4).enumerate().for_each(|(x, pixel)| {
                // let step = x as i32;
                // let world_x = base_x + fixed_mul(floor_step_x, step << FIXED_SHIFT);
                // let world_y = base_y + fixed_mul(floor_step_y, step << FIXED_SHIFT);

                world_x += floor_step_x;
                world_y += floor_step_y;

                let map_x = world_x >> FIXED_SHIFT;
                let map_y = world_y >> FIXED_SHIFT;

                let value = get_grid_value(map_x, map_y, map_width as i32, map_data);
                let has_ceiling = has_bit_set(value, 1);
                let has_road = has_bit_set(value, 3);

                let tex = match (is_floor, has_road, has_ceiling) {
                    (true, false, true) => Some(&floor_texture_data),
                    (false, _, true) => Some(&ceiling_texture_data),
                    (true, true, _) => Some(&road_texture_data),
                    _ => None,
                };

                if let Some(tex) = tex {
                    let frac_x = (world_x & (FIXED_ONE - 1)) as usize;
                    let frac_y = (world_y & (FIXED_ONE - 1)) as usize;

                    let tx = (tex.width as usize * frac_x) >> FIXED_SHIFT;
                    let ty = (tex.height as usize * frac_y) >> FIXED_SHIFT;

                    let tex_idx = (ty * tex.width as usize + tx) * 4;
                    let texel = unsafe { tex.data.get_unchecked(tex_idx..tex_idx + 3) };

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
    point_dx: f32, // position relative to camera
    point_dy: f32,
    height_multiplier: f32,
    width: i32,
    height: i32,
    aspect_ratio: f32,
    inv_det: f32,
) -> TranslationResult {
    let half_height = height / 2;
    let half_width = width / 2;

    // inverse camera matrix calculation
    let transform_x = inv_det * (position.dir_y * point_dx - position.dir_x * point_dy)
        / position.plane_y_initial;
    let transform_y = (inv_det * (-position.plane_y * point_dx + position.plane_x * point_dy))
        / position.plane_y_initial;

    let screen_x = ((half_width as f32) * (1.0 + (transform_x / transform_y))) as i32;

    // to control the pitch/jump
    let v_move_screen =
        position.pitch + ((position.z as f32) / (transform_y * (aspect_ratio * 2.0))) as i32;

    let y_height_before_adjustment = (half_width as f32 / (transform_y)) as i32;
    // since each sprite has a certain height (e.g. 1.1 of the 1 normal height), we multiply by that
    let y_height = (y_height_before_adjustment as f32 * height_multiplier) as i32;

    // how much of a difference there is between height 1 and height of the sprite
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

#[inline(never)]
#[no_mangle]
pub fn draw_sprites_wasm(
    position: &Position,
    img_slice: &mut [u8],
    width: i32,
    height: i32,
    zbuffer: &mut [f32],
    light_range: i32,
    map_light: i32,
    sprites_texture_map: &WasmTextureMap,
    texture_array: &WasmTextureMetaMap,
    found_sprites: &mut SmallVec<[Sprite; 1024]>,
) {
    found_sprites.iter_mut().for_each(|sprite| {
        let dx = sprite.x - position.x;
        let dy = sprite.y - position.y;
        sprite.dx = dx;
        sprite.dy = dy;

        let x_fixed = to_fixed_large(sprite.dx);
        let y_fixed = to_fixed_large(sprite.dy);
        let distance_fixed = (x_fixed).pow(2) + (y_fixed).pow(2);

        sprite.distance_fixed = distance_fixed;
    });
    // since we should draw those in the distance first, we sort them
    found_sprites.sort_unstable_by(|a, b| {
        let da = a.distance_fixed;
        let db = b.distance_fixed;

        db.cmp(&da) // sort descending (farther first)
    });

    // for usage in translate_coordinate_to_camera
    let aspect_ratio = height as f32 / width as f32;
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();

    let sprite_parts_collected: Vec<SpritePart> = found_sprites
        .into_par_iter()
        .filter_map(|sprite| {
            let projection = translate_coordinate_to_camera(
                position,
                sprite.dx,
                sprite.dy,
                sprite.height as f32 / 100.0,
                width,
                height,
                aspect_ratio,
                inv_det,
            );

            let alpha = projection.distance / light_range as f32 - map_light as f32;

            // ensure sprites are always at least a little bit visible - alpha 1 is all black
            let alpha_i = (FIXED_ONE - to_fixed(alpha)).clamp(FIXED_ONE / 8, FIXED_ONE) as i32;

            let texture_meta = texture_array.get(sprite.r#type).unwrap();

            if sprite.r#type == TextureType::WINDOW as i32 {
                let texture_data = sprites_texture_map
                    .get_map()
                    .get(&(sprite.r#type, 0))
                    .unwrap();

                // we'll only run into this when we have a window and a wall in the same coord, but we need to check nevertheless
                if projection.distance > zbuffer[sprite.column as usize] {
                    return None;
                }

                let texture_x: i32 = (sprite.fract * texture_meta.width as f32) as i32;
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
                    full_texture_height: texture_meta.height,
                    full_texture_width: texture_meta.width,
                    full_texture_data: texture_data,
                });
            }

            let angle = atan2(sprite.dx as f64, sprite.dy as f64);

            // will return from -180 to 180
            let angle_i = (((angle).to_degrees() as i32) + 180 + sprite.angle) % 360;

            let mut angle_index = (angle_i) / 45; // Default to 1 if the result is 0

            // if there's no textures for other angles
            if (texture_meta.angles as i32) <= (angle_index) {
                angle_index = 0;
            }
            let texture_data = sprites_texture_map
                .get_map()
                .get(&(sprite.r#type, angle_index))
                .unwrap();

            if projection.distance < 0.0 {
                return None;
            }
            let texture_aspect_ratio = texture_meta.width as f32 / texture_meta.height as f32;

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
            let tex_x1 = ((draw_start_x - to_remove_texture) * texture_meta.width) / sprite_width;
            let tex_width = ((draw_end_x - draw_start_x) * texture_meta.width) / sprite_width;

            Some(SpritePart {
                sprite_type: sprite.r#type,
                sprite_left_x: draw_start_x as u32,
                width: draw_end_x - draw_start_x,
                screen_y_ceiling: projection.screen_y_ceiling,
                height: projection.full_height,
                tex_x1,
                tex_width,
                alpha: alpha_i,
                angle: angle_i,
                full_texture_height: texture_meta.height,
                full_texture_width: texture_meta.width,
                full_texture_data: texture_data,
            })
        })
        .collect();

    img_slice
        .par_chunks_mut(4 * width as usize) // One row at a time
        .enumerate()
        .for_each(|(y, row)| {
            let y = y as i32;
            for sprite in sprite_parts_collected.iter() {
                if y < sprite.screen_y_ceiling || y >= sprite.screen_y_ceiling + sprite.height {
                    continue;
                }
                let dy = y - sprite.screen_y_ceiling;

                let tex_y = dy * sprite.full_texture_height / sprite.height;
                let y_tex_idx = tex_y * sprite.full_texture_width;
                for dx in 0..sprite.width {
                    let tex_x = sprite.tex_x1 + dx * sprite.tex_width / sprite.width;
                    let tex_idx = ((y_tex_idx + tex_x) * 4) as usize;

                    let texel =
                        unsafe { sprite.full_texture_data.get_unchecked(tex_idx..tex_idx + 4) };

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
                        let current_texel = unsafe { row.get_unchecked(idx..idx + 4) };

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

#[inline(never)]
#[no_mangle]
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
    let sky_w_bytes = (sky_width * 4) as usize;
    let screen_w_bytes = (width * 4) as usize;
    let start = ((left_offset * 4) as usize) % sky_w_bytes;
    let end = start + screen_w_bytes;

    img_slice
        .par_chunks_mut((width * 4) as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let screen_y_pitch = y as i32 - position.pitch;
            if screen_y_pitch < 0 || screen_y_pitch >= height {
                return;
            }

            let row_start = screen_y_pitch as usize * sky_w_bytes;

            let idx_start = row_start + start;
            if end <= sky_w_bytes {
                let idx_end = row_start + end;
                if idx_end < pre_scaled_len {
                    row.copy_from_slice(&pre_scaled[idx_start..idx_end]);
                }
            } else {
                let first_part = sky_w_bytes - start;
                let idx_end1 = row_start + sky_w_bytes;
                if idx_end1 < pre_scaled_len {
                    row[..first_part].copy_from_slice(&pre_scaled[idx_start..idx_end1]);
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
        map_x: x.floor() as i32,
        map_y: y.floor() as i32,
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
    let rot_speed = 0.8 * PI * frame_time * multiplier as f32;

    let (sin_r, cos_r) = rot_speed.sin_cos(); // more efficient than separate .sin() and .cos()

    let new_dir_x = dir_x * cos_r - dir_y * sin_r;
    let new_dir_y = dir_x * sin_r + dir_y * cos_r;

    let new_plane_x = plane_x * cos_r - plane_y * sin_r;
    let new_plane_y = plane_x * sin_r + plane_y * cos_r;

    // Avoid heap allocation by creating a fixed-size array
    Float32Array::from(&[new_dir_x, new_dir_y, new_plane_x, new_plane_y][..])
}
