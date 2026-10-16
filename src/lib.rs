#![feature(map_try_insert)]
#![feature(portable_simd)]
use helpers::{
    get_bits, get_grid_value, has_bit_set, to_fixed, BackgroundImageWasm, Position, Sprite,
    SpritePart, Texture, TextureType, TranslationResult, WasmStripePerCoordMap, WasmTextureMap,
    WasmTextureMetaMap, FIXED_ONE, FIXED_SHIFT,
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
#[cfg(test)]
#[path = "tests/render.rs"]
mod render_tests;
mod stairs;
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
    surface_depth_array: *mut f32, // Nearest floor, ceiling or stair depth per pixel
    stair_boxes_array: *const f32, // Grid-derived stair boxes: x0, y0, x1, y1, bottom, top
    stair_box_count: usize,        // Number of boxes in stair_boxes_array
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
    let surface_depth =
        unsafe { from_raw_parts_mut(surface_depth_array, width as usize * height as usize) };
    let stair_boxes = unsafe { from_raw_parts(stair_boxes_array, stair_box_count * 6) };

    let wall_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::WALL as i32))
        .unwrap();

    // Pair a surface's texture pixels with their width and height.
    let surface_texture = |kind: TextureType| {
        let key = kind as i32;
        let meta = sprites_texture_meta_map.get(key).unwrap();
        Texture {
            data: sprites_texture_map.get_map().get(&(key, 0)).unwrap(),
            width: meta.width,
            height: meta.height,
        }
    };
    let mut found_sprites: SmallVec<[Sprite; 1024]> = vec![].into();

    draw_background_image_prescaled(&position, background, img_slice, width, height);
    // Floors at any height use the same projection, coverage and depth rules.
    let planes = [
        FloorPlane {
            height: 0.0,
            facing: Facing::Up,
            cells: 2 | 8, // Ground covers floor cells (bit 1) and road cells (bit 3).
            texture: surface_texture(TextureType::FLOOR),
            alternate: Some((8, surface_texture(TextureType::ROAD))),
            wall_tops: None,
        },
        FloorPlane {
            height: 1.0,
            facing: Facing::Down,
            cells: 2, // Building floor cells also have a ceiling.
            texture: surface_texture(TextureType::CEILING),
            alternate: None,
            wall_tops: None,
        },
        FloorPlane {
            height: 1.0,
            facing: Facing::Up,
            cells: 2, // The same building footprint has a roof above its ceiling.
            texture: surface_texture(TextureType::ROOF),
            alternate: None,
            wall_tops: Some(surface_texture(TextureType::ROOF)),
        },
    ];
    draw_ceiling_floor_raycast(
        &position,
        img_slice,
        surface_depth,
        &planes,
        map_data,
        map_width,
        width,
        height,
        light_range,
        map_light,
    );
    draw_walls_raycast(
        &position,
        zbuffer,
        map_data,
        map_width,
        width,
        range,
        wall_texture_meta.width,
        sprites_map,
        &mut found_sprites,
        z >= height / 2, // Eyes are above wall tops once feet reach half a world unit.
    );
    stairs::draw(
        &position,
        img_slice,
        surface_depth,
        stair_boxes,
        width,
        height,
        surface_texture(TextureType::STAIRS),
        light_range,
        map_light,
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
        surface_depth,
    );
}

pub fn raycast_column(
    column: i32,
    position: &Position,
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width: i32,
    range: i8,
    wall_texture_width: i32,
    sprites_map: Option<&HashMap<(i32, i32), Vec<Sprite>>>,
    skip_sprites_and_writes: bool,
    stop_at_window: bool,
    above_walls: bool, // Continue past nearby walls to find walls visible beyond their tops.
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
    let initial_bit_offset = 16;

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
        // Stair cells (bit 10) have zero walls; their upper bits are flight
        // metadata, not wall segments. TypeScript derives their surface boxes.
        let num_walls = get_bits(value, 12);

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
                            hit = true;
                            distance_multiplier = local_distance_multiplier;
                        }
                        // switch which side we were raycasting from to take the fract part to know where the texture was hit
                        let mut fract: f32;
                        if local_side == 1 {
                            fract = local_intersection_coord.x.fract();
                        } else {
                            fract = local_intersection_coord.y.fract();
                        }

                        let texture_type;

                        // has door bit set
                        if is_door {
                            hit_type = 0x2 as i8;
                            texture_type = TextureType::DOOR as i32;
                            // since we'd like the texture to match the width
                            fract -= local_offset;
                            fract /= local_width;
                        } else if is_window {
                            hit_type = 0x3 as i8;
                            texture_type = TextureType::WINDOW as i32;
                            // since we'd like the texture to match the width
                            fract -= local_offset;
                            fract /= local_width;
                        } else {
                            hit_type = 1;
                            texture_type = TextureType::WALL as i32;
                        }

                        // add to visible sprites
                        if !skip_sprites_and_writes {
                            window_sprites.push(Sprite {
                                x: local_intersection_coord.x,
                                y: local_intersection_coord.y,
                                angle: 0,
                                height: 100,
                                r#type: texture_type as i32,
                                column: column as u32,
                                distance: local_distance,
                                distance_fixed: 0,
                                dx: 0.,
                                dy: 0.,
                                fract,
                            });
                        }
                    }
                }
            }
            if hit && !above_walls {
                side_dist_x += delta_dist_x * distance_multiplier;
                side_dist_y += delta_dist_y * distance_multiplier;
            }
        }

        // handle thick wall
        if value == 1 {
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
        if hit && !above_walls {
            break;
        }
        // From above, a wall hit must not stop the ray's search through farther cells.
        if above_walls {
            hit = false;
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

    perp_wall_dist = perp_wall_dist * position.plane_y_initial;

    let col_data = [0, column, 0, 0, 0, hit as i32, hit_type as i32];

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
    zbuffer: &mut [f32],
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width: i32,
    range: i8,
    wall_texture_width: i32,
    sprites_map: &WasmStripePerCoordMap,
    found_sprites: &mut SmallVec<[Sprite; 1024]>,
    above_walls: bool,
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
                range,
                wall_texture_width,
                Some(&sprites_map.get_map()),
                false,
                false,
                above_walls,
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
    // since sprites don't have a distance yet, do it now
    found_sprites.iter_mut().for_each(|sprite| {
        let local_distance = Euclidean.distance(
            Coord::from([position.x, position.y]),
            Coord::from([sprite.x, sprite.y]),
        );
        sprite.distance = local_distance;
    });

    for (idx, (perp_wall_dist, _, _, window_sprites)) in data.iter().enumerate() {
        // Above walls, per-pixel roof depth replaces whole-column wall occlusion.
        zbuffer[idx] = if above_walls {
            f32::INFINITY
        } else {
            *perp_wall_dist
        };
        found_sprites.extend((*window_sprites).clone());
    }
}

#[derive(Clone, Copy)]
pub enum Facing {
    Up,
    Down,
}

pub struct FloorPlane<'a> {
    pub height: f32,                           // World height of this floor or ceiling
    pub facing: Facing,                        // Visible side of the plane
    pub cells: u64,                            // Grid flags marking where the plane exists
    pub texture: Texture<'a>,                  // Default surface material
    pub alternate: Option<(u64, Texture<'a>)>, // Optional material selected by a grid flag
    pub wall_tops: Option<Texture<'a>>,        // Material for exposed wall tops at this height
}

fn inside_wall(value: u64, x: f32, y: f32) -> bool {
    // Coordinates are local to this cell; value 1 fills the whole cell.
    if value == 1 {
        return true;
    }
    for index in 0..get_bits(value, 12).min(3) {
        // Each wall footprint uses four 4-bit fields, measured in tenths of a cell.
        let shift = 16 + index * 16;
        let primary = (get_bits(value, shift) % 11) as f32 / 10.0;
        let thickness = (get_bits(value, shift + 4) % 11) as f32 / 10.0;
        let length = (get_bits(value, shift + 8) % 11) as f32 / 10.0;
        let secondary = (get_bits(value, shift + 12) % 11) as f32 / 10.0;
        // These are the existing orientation bits for the cell's three wall slots.
        let (along, across) = if has_bit_set(value, [6, 7, 2][index as usize]) {
            (x, y)
        } else {
            (y, x)
        };
        if across >= primary
            && across < primary + thickness
            && along >= secondary
            && along < secondary + length
        {
            return true;
        }
    }
    false
}

pub fn draw_ceiling_floor_raycast(
    position: &Position,
    pixels: &mut [u8],
    depth: &mut [f32],
    planes: &[FloorPlane<'_>],
    grid: &[u64],
    size: usize,
    width: i32,
    height: i32,
    light_range: i32,
    map_light: i32,
) {
    // Convert feet to world units and add the fixed half-unit eye height.
    let eye = 0.5 + position.z as f32 / height as f32;
    let middle = height as f32 * 0.5 + position.pitch as f32;
    pixels
        .par_chunks_mut(width as usize * 4)
        .zip(depth.par_chunks_mut(width as usize))
        .enumerate()
        .for_each(|(y, (row, zrow))| {
            // Start each frame with no horizontal surface hiding this row.
            zrow.fill(f32::INFINITY);
            // Sample pixel centers relative to the pitched horizon.
            let screen_offset = y as f32 + 0.5 - middle;
            // Nearer planes are normally declared last; depth also makes order safe.
            for plane in planes.iter().rev() {
                // Floor/roof tops face up; ceilings face down.
                let visible = match plane.facing {
                    Facing::Up => eye > plane.height,
                    Facing::Down => eye < plane.height,
                };
                if !visible {
                    continue;
                }
                // A flat plane has the same forward depth across an entire screen row.
                let distance = (eye - plane.height) * width as f32 * 0.5 / screen_offset;
                let ray_distance = distance / position.plane_y_initial;
                // Skip the horizon and intersections behind or too close to the camera.
                if !distance.is_finite() || ray_distance <= 0.02 {
                    continue;
                }
                let light = ((1.0 - distance / light_range as f32 + map_light as f32)
                    .clamp(0.0, 1.0)
                    * 256.0) as u16;
                // Wide fixed coordinates avoid overflow near the horizon on large views.
                let step_x = (ray_distance * 2.0 * position.plane_x / width as f32
                    * FIXED_ONE as f32) as i64;
                let step_y = (ray_distance * 2.0 * position.plane_y / width as f32
                    * FIXED_ONE as f32) as i64;
                let mut world_x = ((position.x
                    + ray_distance
                        * (position.dir_x - position.plane_x + position.plane_x / width as f32))
                    * FIXED_ONE as f32) as i64;
                let mut world_y = ((position.y
                    + ray_distance
                        * (position.dir_y - position.plane_y + position.plane_y / width as f32))
                    * FIXED_ONE as f32) as i64;
                for (pixel, z) in row.chunks_exact_mut(4).zip(zrow.iter_mut()) {
                    let value = get_grid_value(
                        (world_x >> FIXED_SHIFT) as i32,
                        (world_y >> FIXED_SHIFT) as i32,
                        size as i32,
                        grid,
                    );
                    // A nearer plane may already cover this pixel.
                    if distance < *z {
                        let texture = if value & plane.cells != 0 {
                            Some(match &plane.alternate {
                                Some((flag, texture)) if value & flag != 0 => texture,
                                _ => &plane.texture,
                            })
                        } else {
                            // Without a full roof cell, draw only the actual wall footprint.
                            plane.wall_tops.as_ref().filter(|_| {
                                inside_wall(
                                    value,
                                    (world_x & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                                    (world_y & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                                )
                            })
                        };
                        if let Some(texture) = texture {
                            // Fractional world coordinates repeat the texture once per cell.
                            let tx = (texture.width as usize
                                * (world_x & (FIXED_ONE as i64 - 1)) as usize)
                                >> FIXED_SHIFT;
                            let ty = (texture.height as usize
                                * (world_y & (FIXED_ONE as i64 - 1)) as usize)
                                >> FIXED_SHIFT;
                            let idx = (ty * texture.width as usize + tx) * 4;
                            for c in 0..3 {
                                pixel[c] = (texture.data[idx + c] as u16 * light >> 8) as u8;
                            }
                            *z = distance;
                        }
                    }
                    world_x += step_x;
                    world_y += step_y;
                }
            }
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
        distance: transform_y, // this is the perpendicular, not euclidian distance (!)
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
    surface_depth: &[f32],
) {
    found_sprites.iter_mut().for_each(|sprite| {
        let dx = sprite.x - position.x;
        let dy = sprite.y - position.y;
        sprite.dx = dx;
        sprite.dy = dy;
    });
    // since we should draw those in the distance first, we sort them
    found_sprites.sort_unstable_by(|a, b| {
        b.distance.partial_cmp(&a.distance).unwrap() // sort descending (farther first)
    });

    // for usage in translate_coordinate_to_camera
    let aspect_ratio = height as f32 / width as f32;
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();

    let sprite_parts_collected: Vec<SpritePart> = found_sprites
        .into_iter()
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
            // Reject sprites behind the camera or too small to draw safely.
            if projection.distance <= 0.02 || projection.full_height <= 0 {
                return None;
            }
            // Only inspect rows where the sprite actually reaches the screen.
            let visible_start = projection.screen_y_ceiling.max(0) as usize;
            let visible_end =
                (projection.screen_y_ceiling + projection.full_height).clamp(0, height) as usize;
            if visible_start >= visible_end {
                return None;
            }

            let alpha = projection.distance / light_range as f32 - map_light as f32;

            // ensure sprites are always at least a little bit visible - alpha 1 is all black
            let alpha_i = (FIXED_ONE - to_fixed(alpha)).clamp(FIXED_ONE / 8, FIXED_ONE) as i32;

            let texture_meta = texture_array.get(sprite.r#type).unwrap();

            if sprite.r#type == TextureType::WINDOW as i32
                || sprite.r#type == TextureType::DOOR as i32
                || sprite.r#type == TextureType::WALL as i32
            {
                // Skip wall stripes fully hidden by nearer floors or roofs.
                if (visible_start..visible_end).all(|y| {
                    surface_depth[y * width as usize + sprite.column as usize]
                        <= projection.distance
                }) {
                    return None;
                }
                let texture_data = sprites_texture_map
                    .get_map()
                    .get(&(sprite.r#type, 0))
                    .unwrap();

                let texture_x: i32 = (sprite.fract * texture_meta.width as f32) as i32;

                let inv_sprite_height = texture_meta.height * FIXED_ONE / projection.full_height;
                let inv_sprite_width = 1 * FIXED_ONE / (1);
                return Some(SpritePart {
                    depth: projection.distance,
                    sprite_type: sprite.r#type,
                    sprite_left_x: sprite.column,
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
                    inv_sprite_height,
                    inv_sprite_width,
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

            let texture_aspect_ratio = texture_meta.width as f32 / texture_meta.height as f32;

            let sprite_width = (projection.full_height as f32 * texture_aspect_ratio as f32) as i32;
            if sprite_width <= 0 {
                return None;
            }

            let mut draw_start_x = (-sprite_width / 2 + projection.screen_x).max(0);
            let mut draw_end_x = (sprite_width / 2 + projection.screen_x).min(width - 1);
            if draw_start_x >= draw_end_x {
                return None;
            }

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

            let inv_sprite_height = texture_meta.height * FIXED_ONE / projection.full_height;
            let inv_sprite_width = tex_width * FIXED_ONE / (draw_end_x - draw_start_x).max(1);
            Some(SpritePart {
                depth: projection.distance,
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
                inv_sprite_height,
                inv_sprite_width,
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
                let tex_y = ((dy * sprite.inv_sprite_height) >> FIXED_SHIFT) as i32;
                let y_tex_idx = tex_y * sprite.full_texture_width;

                for dx in 0..sprite.width {
                    let tex_x =
                        sprite.tex_x1 + ((dx * sprite.inv_sprite_width) >> FIXED_SHIFT) as i32;
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
                    // A roof can hide only part of a tree or wall, so check each pixel.
                    if sprite.depth >= surface_depth[y as usize * width as usize + x as usize] {
                        continue;
                    }

                    // alpha blending
                    if a != 255 {
                        let current_texel = unsafe { row.get_unchecked(idx..idx + 3) };

                        let inverted_alpha = 255 - a;
                        r = (((a * r as u16) + (current_texel[0] as u16 * inverted_alpha)) >> 8)
                            as u8;
                        g = (((a * g as u16) + (current_texel[1] as u16 * inverted_alpha)) >> 8)
                            as u8;
                        b = (((a * b as u16) + (current_texel[2] as u16 * inverted_alpha)) >> 8)
                            as u8;
                    }

                    let dst = unsafe { row.get_unchecked_mut(idx..idx + 3) };
                    // dst.copy_from_slice(&[r, g, b]);
                    dst[0] = r;
                    dst[1] = g;
                    dst[2] = b;
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
            // Extend panorama edges when pitching so no pixels retain the previous frame.
            let screen_y_pitch = (y as i32 - position.pitch).clamp(0, height - 1);

            let row_start = screen_y_pitch as usize * sky_w_bytes;

            let idx_start = row_start + start;
            if end <= sky_w_bytes {
                let idx_end = row_start + end;
                if idx_end <= pre_scaled_len {
                    row.copy_from_slice(&pre_scaled[idx_start..idx_end]);
                }
            } else {
                let first_part = sky_w_bytes - start;
                let idx_end1 = row_start + sky_w_bytes;
                if idx_end1 <= pre_scaled_len {
                    row[..first_part].copy_from_slice(&pre_scaled[idx_start..idx_end1]);
                }

                let idx_start2 = row_start;
                let idx_end2 = row_start + (screen_w_bytes - first_part);
                if idx_end2 <= pre_scaled_len {
                    row[first_part..].copy_from_slice(&pre_scaled[idx_start2..idx_end2]);
                }
            }
        });
}

// Roof coverage matches the floor renderer, including exposed thin-wall tops.
#[wasm_bindgen]
pub fn roof_height(x: f32, y: f32, map_array: *const u64, map_width: usize) -> f32 {
    let grid = unsafe { from_raw_parts(map_array, map_width * map_width) };
    roof_height_at(x, y, grid, map_width)
}

fn roof_height_at(x: f32, y: f32, grid: &[u64], size: usize) -> f32 {
    let value = get_grid_value(x.floor() as i32, y.floor() as i32, size as i32, grid);
    // Bit 1 covers the building's roof; other walls support feet only on their tops.
    if value & 2 != 0 || inside_wall(value, x - x.floor(), y - y.floor()) {
        1.0
    } else {
        0.0
    }
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
        range,
        wall_texture_width,
        None,
        true,
        true,
        false,
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
        range,
        wall_texture_width,
        None,
        true,
        true,
        false,
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
        range,
        wall_texture_width,
        None,
        true,
        true,
        false,
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

#[cfg(test)]
#[path = "tests/floors.rs"]
mod floor_tests;

#[cfg(test)]
#[path = "tests/roofs.rs"]
mod roof_tests;
