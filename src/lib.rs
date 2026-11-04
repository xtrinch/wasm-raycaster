#![feature(map_try_insert)]
#![feature(portable_simd)]
use helpers::{
    get_bits, get_grid_value, has_bit_set, to_fixed, to_fixed_i64, BackgroundImageWasm,
    CollisionBox, CollisionCells, Position, ScreenTriangle, Sprite, SpritePart, Texture,
    TextureType, TranslationResult, WasmTextureMap, WasmTextureMetaMap, WasmWorld, FIXED_ONE,
    FIXED_SHIFT,
};
use js_sys::Float32Array;
use js_sys::Math::atan2;
use smallvec::SmallVec;
use wasm_bindgen::prelude::*;
use world::World;

use rayon::prelude::*;

#[cfg(feature = "parallel")]
pub use wasm_bindgen_rayon::init_thread_pool;

mod helpers;
mod line_intersection;
mod world;
mod world_helpers;
use geo::{Coord, Distance, Euclidean, Line};
use line_intersection::LineInterval;
use std::collections::HashSet;
use std::f32::consts::PI;
use std::{collections::HashMap, f32::MAX};
use web_sys::console;
// let js: JsValue = vec![found_sprites_length as f32].into();
// console::log_2(&"Znj?".into(), &js);
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
    z: f32,
    plane_y_initial: f32,
    render_img: *mut u8,
    column_depth_array: *mut f32,
    world: &WasmWorld, // grid, sprites and models
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    map_light: i32,
    background: &BackgroundImageWasm,
    sprites_texture_map: &WasmTextureMap, // contains textures along with angled textures
    sprites_texture_meta_map: &WasmTextureMetaMap,
    depth_array: *mut f32, // Nearest floor, ceiling, roof or mesh depth per pixel
) {
    let world = world.world();
    let map_data = world.grid();
    let map_width = world.size();
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

    let column_depth = unsafe { from_raw_parts_mut(column_depth_array, width as usize) };
    // One per-pixel depth buffer, built up by the passes in order: the floor pass resets it
    // and stores floor, ceiling and roof depth; the mesh pass adds nearer mesh pixels; the
    // sprite pass then skips wall, window and sprite pixels behind either.
    let depth = unsafe { from_raw_parts_mut(depth_array, width as usize * height as usize) };

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

    let roof_texture_meta = sprites_texture_meta_map
        .get_map()
        .get(&(TextureType::ROOF as i32))
        .unwrap();
    let roof_texture = sprites_texture_map
        .get_map()
        .get(&(TextureType::ROOF as i32, 0))
        .unwrap();

    // Filled by the wall raycast from the cells its rays reach: sprites (and window
    // glass) for the column-based sprite pass, model ids for the triangle mesh pass.
    let mut found_sprites: SmallVec<[Sprite; 1024]> = vec![].into();
    let mut found_models: Vec<u32> = Vec::new();

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
        roof_texture,
        roof_texture_meta.width,
        roof_texture_meta.height,
        depth,
    );
    draw_walls_raycast(
        &position,
        column_depth,
        map_data,
        map_width,
        width,
        range,
        wall_texture_meta.width,
        world,
        &mut found_sprites,
        &mut found_models,
    );
    // Only models in cells the rays reached are drawn, like sprites.
    let visible_meshes: Vec<&[f32]> = found_models
        .iter()
        .filter_map(|&model| world.model_triangles(model))
        .collect();
    // Meshes go first so walls, sprites and window glass in front of them draw over them.
    draw_meshes(
        &position,
        img_slice,
        width,
        height,
        &visible_meshes,
        depth,
        light_range,
        map_light,
    );
    draw_sprites_wasm(
        &position,
        img_slice,
        width,
        height,
        column_depth,
        light_range,
        map_light,
        sprites_texture_map,
        sprites_texture_meta_map,
        &mut found_sprites,
        depth,
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
    render_cells: Option<&World>, // records cells holding sprites or models
    skip_sprites_and_writes: bool,
    stop_at_window: bool,
    above_walls: bool, // Continue past nearby walls to find walls visible beyond their tops.
) -> (f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[Sprite; 2]>) {
    let mut occupied_cells: Vec<(i32, i32)> = Vec::new();
    let mut segment_hits: SmallVec<[Sprite; 2]> = SmallVec::with_capacity(2);

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
                        is_window = has_bit_set(value, 9);
                    }
                    1 => {
                        bit_width = get_bits(value, initial_bit_offset + 24);
                        bit_offset = get_bits(value, initial_bit_offset + 16);
                        bit_thickness = get_bits(value, initial_bit_offset + 20);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 28);
                        is_door = has_bit_set(value, 4);
                        is_east = !has_bit_set(value, 7);
                        is_window = has_bit_set(value, 10);
                    }
                    2 => {
                        bit_width = get_bits(value, initial_bit_offset + 40);
                        bit_offset = get_bits(value, initial_bit_offset + 32);
                        bit_thickness = get_bits(value, initial_bit_offset + 36);
                        bit_offset_secondary = get_bits(value, initial_bit_offset + 44);
                        is_door = has_bit_set(value, 4);
                        is_east = !has_bit_set(value, 8);
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
                    // Above wall tops, farther segments in this cell can still be visible.
                    if local_distance < distance || above_walls {
                        // we'll only use this data if we're stopping at a window or it's not a window
                        if (!is_window || stop_at_window) && local_distance < distance {
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
                            segment_hits.push(Sprite {
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

        // only add coord if sprites or models exist in it;
        // TODO: check more smartly
        if !skip_sprites_and_writes && column % 5 == 0 {
            if render_cells.is_some_and(|cells| cells.occupied(&(map_x, map_y))) {
                let _ = occupied_cells.push((map_x, map_y));
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
        occupied_cells.to_vec(),
        segment_hits,
    )
}

#[inline(never)]
#[no_mangle]
pub fn draw_walls_raycast(
    position: &Position,
    column_depth: &mut [f32],
    map_data: &[u64],
    map_width: usize, // Needed to index into 1D map
    width: i32,
    range: i8,
    wall_texture_width: i32,
    render_cells: &World,
    found_sprites: &mut SmallVec<[Sprite; 1024]>,
    found_models: &mut Vec<u32>, // models in cells the rays reached, each listed once
) {
    // Eyes are above wall tops once feet reach half a world unit.
    let above_walls = position.z >= 0.5;
    let data: Vec<(f32, [i32; 7], Vec<(i32, i32)>, SmallVec<[Sprite; 2]>)> = (0..width)
        .into_par_iter()
        .map(|column| {
            let (perp_wall_dist, col_data, occupied_cells, segment_hits) = raycast_column(
                column,
                position,
                map_data,
                map_width,
                width,
                range,
                wall_texture_width,
                Some(render_cells),
                false,
                false,
                above_walls,
            );

            (perp_wall_dist, col_data, occupied_cells, segment_hits)
        })
        .collect();

    let reached_cells: HashSet<(i32, i32)> = data
        .iter()
        .flat_map(|(_, _, occupied_cells, _)| occupied_cells.iter().copied())
        .collect();

    for cell in &reached_cells {
        found_sprites.extend(render_cells.sprites_at(cell).iter().cloned());
        found_models.extend_from_slice(render_cells.models_at(cell));
    }
    // A model covering several reached cells is drawn once.
    found_models.sort_unstable();
    found_models.dedup();
    // since sprites don't have a distance yet, do it now
    found_sprites.iter_mut().for_each(|sprite| {
        let local_distance = Euclidean.distance(
            Coord::from([position.x, position.y]),
            Coord::from([sprite.x, sprite.y]),
        );
        sprite.distance = local_distance;
    });

    for (idx, (perp_wall_dist, _, _, segment_hits)) in data.iter().enumerate() {
        // Above walls, per-pixel roof depth replaces whole-column wall occlusion.
        column_depth[idx] = if above_walls {
            f32::INFINITY
        } else {
            *perp_wall_dist
        };
        found_sprites.extend((*segment_hits).clone());
    }
}

// Check the shared wall-bottom/top footprint using cell-local coordinates.
// A cell's wall, door and window slots as local rectangles: min x, min y, max x, max y in
// cell units, and whether the slot is a door. A cell whose value is exactly 1 is one
// full-cell wall.
fn wall_slots(value: u64) -> SmallVec<[([f32; 4], bool); 3]> {
    if value == 1 {
        return SmallVec::from_slice(&[([0.0, 0.0, 1.0, 1.0], false)]);
    }
    (0..get_bits(value, 12).min(3))
        .map(|index| {
            // Each wall footprint uses four 4-bit fields, measured in tenths of a cell.
            let shift = 16 + index * 16;
            let primary = (get_bits(value, shift) % 11) as f32 / 10.0;
            let thickness = (get_bits(value, shift + 4) % 11) as f32 / 10.0;
            let length = (get_bits(value, shift + 8) % 11) as f32 / 10.0;
            let secondary = (get_bits(value, shift + 12) % 11) as f32 / 10.0;
            let door = has_bit_set(value, if index == 0 { 5 } else { 4 });
            // These are the existing orientation bits for the cell's three wall slots.
            let rect = if has_bit_set(value, [6, 7, 8][index as usize]) {
                [secondary, primary, secondary + length, primary + thickness]
            } else {
                [primary, secondary, primary + thickness, secondary + length]
            };
            (rect, door)
        })
        .collect()
}

fn inside_wall_footprint(value: u64, x: f32, y: f32) -> bool {
    // Coordinates are local to this cell.
    wall_slots(value)
        .iter()
        .any(|(r, _)| x >= r[0] && x < r[2] && y >= r[1] && y < r[3])
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
    roof_texture_array: &[u8], // Concrete pixels for roofs and wall tops.
    roof_texture_width: i32,
    roof_texture_height: i32,
    depth: &mut [f32], // Nearest horizontal surface per screen pixel.
) {
    let ray_dir_x0 = position.dir_x - position.plane_x;
    let ray_dir_y0 = position.dir_y - position.plane_y;
    let ray_dir_x1 = position.dir_x + position.plane_x;
    let ray_dir_y1 = position.dir_y + position.plane_y;
    let ray_dir_x_dist = ray_dir_x1 - ray_dir_x0;
    let ray_dir_y_dist = ray_dir_y1 - ray_dir_y0;
    // Eyes sit half a world unit above the player's feet.
    let eye = 0.5 + position.z;
    let middle_view_y = height as f32 * 0.5 + position.pitch as f32;

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
    let roof_texture_data = Texture {
        data: roof_texture_array,
        width: roof_texture_width,
        height: roof_texture_height,
    };

    img_slice
        .par_chunks_mut(width as usize * 4)
        .zip(depth.par_chunks_mut(width as usize))
        .enumerate()
        .for_each(|(y, (row, zrow))| {
            // Start each frame with no horizontal surface hiding this row.
            zrow.fill(f32::INFINITY);
            // Sample pixel centers relative to the pitched horizon.
            let p = y as f32 + 0.5 - middle_view_y;
            // Current geometry has only ground at 0 and ceiling/roof at wall-top height 1.
            for surface_height in [1.0, 0.0] {
                let is_floor = eye > surface_height;
                let is_ground = surface_height == 0.0;
                let is_roof = !is_ground && is_floor;
                // Ground has no underside; upper boundaries are ceilings from below.
                if is_ground && !is_floor {
                    continue;
                }
                // A flat surface has the same forward depth across an entire screen row.
                let distance = (eye - surface_height) * width as f32 * 0.5 / p;
                let row_distance = distance / position.plane_y_initial;
                // Skip the horizon and intersections behind or too close to the camera.
                if !distance.is_finite() || row_distance <= 0.02 {
                    continue;
                }
                let alpha = ((1.0 - distance / light_range as f32 + map_light as f32)
                    .clamp(0.0, 1.0)
                    * 256.0) as u16;
                // Wide fixed coordinates avoid overflow near the horizon on large views.
                let floor_step_x = to_fixed_i64(row_distance * ray_dir_x_dist / width as f32);
                let floor_step_y = to_fixed_i64(row_distance * ray_dir_y_dist / width as f32);
                let base_x = to_fixed_i64(
                    position.x + row_distance * (ray_dir_x0 + position.plane_x / width as f32),
                );
                let base_y = to_fixed_i64(
                    position.y + row_distance * (ray_dir_y0 + position.plane_y / width as f32),
                );
                let mut world_x = base_x;
                let mut world_y = base_y;

                for (pixel, z) in row.chunks_exact_mut(4).zip(zrow.iter_mut()) {
                    let map_x = (world_x >> FIXED_SHIFT) as i32;
                    let map_y = (world_y >> FIXED_SHIFT) as i32;
                    let value = get_grid_value(map_x, map_y, map_width as i32, map_data);
                    // A nearer plane may already cover this pixel.
                    if distance < *z {
                        let has_floor = has_bit_set(value, 1);
                        let has_ceiling = has_bit_set(value, 2);
                        let has_road = has_bit_set(value, 3);
                        // Wall bottoms and tops share one footprint; full-cell surfaces need no check.
                        let on_wall = ((is_ground && !has_floor && !has_road)
                            || (is_roof && !has_ceiling))
                            && inside_wall_footprint(
                                value,
                                (world_x & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                                (world_y & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                            );
                        let tex = match (is_floor, has_road, has_floor, has_ceiling) {
                            (true, _, _, true) if is_roof => Some(&roof_texture_data),
                            (true, _, _, _) if is_roof && on_wall => Some(&roof_texture_data),
                            (false, _, _, true) => Some(&ceiling_texture_data),
                            (true, true, _, _) if is_ground => Some(&road_texture_data),
                            (true, _, true, _) if is_ground => Some(&floor_texture_data),
                            (true, _, _, _) if is_ground && on_wall => Some(&floor_texture_data),
                            _ => None,
                        };
                        if let Some(tex) = tex {
                            // Fractional world coordinates repeat the texture once per cell.
                            let frac_x = (world_x & (FIXED_ONE as i64 - 1)) as usize;
                            let frac_y = (world_y & (FIXED_ONE as i64 - 1)) as usize;
                            let tx = (tex.width as usize * frac_x) >> FIXED_SHIFT;
                            let ty = (tex.height as usize * frac_y) >> FIXED_SHIFT;

                            let tex_idx = (ty * tex.width as usize + tx) * 4;
                            let texel = &tex.data[tex_idx..tex_idx + 3];
                            pixel[0] = (texel[0] as u16 * alpha >> 8) as u8;
                            pixel[1] = (texel[1] as u16 * alpha >> 8) as u8;
                            pixel[2] = (texel[2] as u16 * alpha >> 8) as u8;
                            *z = distance;
                        }
                    }
                    world_x += floor_step_x;
                    world_y += floor_step_y;
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
    let v_move_screen = position.pitch
        + ((position.z * height as f32) / (transform_y * (aspect_ratio * 2.0))) as i32;

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
    column_depth: &mut [f32],
    light_range: i32,
    map_light: i32,
    sprites_texture_map: &WasmTextureMap,
    texture_array: &WasmTextureMetaMap,
    found_sprites: &mut SmallVec<[Sprite; 1024]>,
    depth: &[f32], // floor, ceiling, roof and mesh depth per pixel
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
                // Skip wall stripes fully hidden by nearer floors, roofs or meshes.
                if (visible_start..visible_end).all(|y| {
                    depth[y * width as usize + sprite.column as usize] <= projection.distance
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
                && (projection.distance >= column_depth[idx_start as usize])
                && draw_start_x + 1 < draw_end_x
            {
                draw_start_x += 1;
                idx_start += 1;
            }

            let mut idx_end = draw_end_x;
            while idx_end >= 0
                && (projection.distance >= column_depth[idx_end as usize])
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
                    // A roof or mesh can hide only part of a tree or wall, so check each pixel.
                    let pixel = y as usize * width as usize + x as usize;
                    if sprite.depth >= depth[pixel] {
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

// Each mesh triangle is three vertices of world-space x, y, z and normal x, y, z,
// followed by the triangle's r, g, b (0-1).
pub const MESH_TRIANGLE_STRIDE: usize = 21;
// Triangles are clipped at this camera depth so vertices behind the eye never project.
const MESH_NEAR_DEPTH: f32 = 0.05;
// Screen rows per rasterization band; each band is filled by one thread.
const MESH_BAND_ROWS: usize = 8;

// Signed doubled area of (a, b, p); its sign tells which side of a->b the point is on.
#[inline(always)]
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

// Clip a camera-space polygon (lateral, depth, height, shade) to depth >= MESH_NEAR_DEPTH.
fn clip_to_near_plane(vertices: &[[f32; 4]]) -> SmallVec<[[f32; 4]; 4]> {
    let mut clipped = SmallVec::new();
    for i in 0..vertices.len() {
        let a = vertices[i];
        let b = vertices[(i + 1) % vertices.len()];
        let a_inside = a[1] >= MESH_NEAR_DEPTH;
        let b_inside = b[1] >= MESH_NEAR_DEPTH;
        if a_inside {
            clipped.push(a);
        }
        if a_inside != b_inside {
            let t = (MESH_NEAR_DEPTH - a[1]) / (b[1] - a[1]);
            clipped.push([0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * t));
        }
    }
    clipped
}

// Whether a mesh triangle's outside faces away from the eye. On a closed model such faces
// are always covered by its near side. The vertex normals decide which side is outside,
// so winding order does not matter.
fn faces_away_from(t: &[f32], eye: [f32; 3]) -> bool {
    let vertex = |i: usize| [t[i * 6], t[i * 6 + 1], t[i * 6 + 2]];
    let (a, b, c) = (vertex(0), vertex(1), vertex(2));
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let face = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];
    let outward: f32 = (0..3)
        .map(|k| face[k] * (t[3 + k] + t[9 + k] + t[15 + k]))
        .sum();
    let towards_triangle: f32 = (0..3).map(|k| face[k] * (a[k] - eye[k])).sum();
    // Same sign: the outside normal points along the view ray, i.e. away from the eye.
    outward * towards_triangle > 0.0
}

// Project world-space triangles with the sprite camera transform and fill them per pixel.
// Tests and writes the shared depth buffer, which already holds floors, ceilings and roofs;
// draw_sprites_wasm runs after it and skips wall and sprite pixels behind any of them.
#[inline(never)]
pub fn draw_meshes(
    position: &Position,
    img_slice: &mut [u8],
    width: i32,
    height: i32,
    meshes: &[&[f32]], // each a run of triangles in MESH_TRIANGLE_STRIDE layout
    depth_buffer: &mut [f32], // per-pixel depth of floors, ceilings and roofs; meshes add theirs
    light_range: i32,
    map_light: i32,
) {
    if meshes.iter().all(|mesh| mesh.len() < MESH_TRIANGLE_STRIDE) {
        return;
    }
    let half_width = (width / 2) as f32;
    let middle_view_y = height as f32 * 0.5 + position.pitch as f32;
    let eye = position.z + 0.5;
    // Same scale as translate_coordinate_to_camera, so depths compare with walls and sprites.
    let scale = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs()
        / position.plane_y_initial;
    // Fixed sun direction; surfaces are lit from either side so winding order does not matter.
    let light = [0.4, 0.3, 0.866_f32];

    let screen_triangles: Vec<ScreenTriangle> = meshes
        .par_iter()
        .flat_map(|mesh| mesh.par_chunks_exact(MESH_TRIANGLE_STRIDE))
        .flat_map_iter(|t| {
            // Camera-space vertices with their light level; normals are unit length.
            let camera: [[f32; 4]; 3] = [0, 1, 2].map(|i| {
                let v = &t[i * 6..i * 6 + 6];
                let dx = v[0] - position.x;
                let dy = v[1] - position.y;
                let lit = (v[3] * light[0] + v[4] * light[1] + v[5] * light[2]).abs();
                [
                    scale * (position.dir_y * dx - position.dir_x * dy),
                    scale * (-position.plane_y * dx + position.plane_x * dy),
                    v[2],
                    0.35 + 0.65 * lit.min(1.0),
                ]
            });

            // Skip triangles entirely behind the eye or beside the view (each test is a
            // half-space through the eye that contains no visible pixel).
            let outside = camera.iter().all(|p| p[1] < MESH_NEAR_DEPTH)
                || camera.iter().all(|p| p[0] > p[1])
                || camera.iter().all(|p| p[0] < -p[1]);
            let clipped = if outside || faces_away_from(t, [position.x, position.y, eye]) {
                SmallVec::new()
            } else {
                clip_to_near_plane(&camera)
            };

            // Darken with distance like sprites do, using the triangle's average depth.
            let depth = camera.iter().map(|p| p[1]).sum::<f32>() / 3.0;
            let fog = (1.0 - (depth / light_range as f32 - map_light as f32)).clamp(0.125, 1.0);
            let color = [t[18], t[19], t[20]].map(|c| c * fog * 255.0);

            // Screen x, y, 1 / depth and shade / depth, which interpolate linearly on screen.
            let projected: SmallVec<[[f32; 4]; 4]> = clipped
                .iter()
                .map(|p| {
                    [
                        half_width * (1.0 + p[0] / p[1]),
                        middle_view_y + (half_width / p[1]) * (eye - p[2]),
                        1.0 / p[1],
                        p[3] / p[1],
                    ]
                })
                .collect();

            // Fan-triangulate the clipped polygon (3 or 4 vertices).
            (1..projected.len().saturating_sub(1)).filter_map(move |i| {
                let corners = [projected[0], projected[i], projected[i + 1]];
                let xs = corners.map(|c| c[0]);
                let ys = corners.map(|c| c[1]);
                let min_x = (xs.iter().cloned().fold(f32::MAX, f32::min).floor() as i32).max(0);
                let max_x =
                    (xs.iter().cloned().fold(f32::MIN, f32::max).ceil() as i32).min(width - 1);
                let min_y = (ys.iter().cloned().fold(f32::MAX, f32::min).floor() as i32).max(0);
                let max_y =
                    (ys.iter().cloned().fold(f32::MIN, f32::max).ceil() as i32).min(height - 1);
                if min_x > max_x || min_y > max_y {
                    return None;
                }
                // Dividing by the signed area accepts either winding.
                let area = edge(xs[0], ys[0], xs[1], ys[1], xs[2], ys[2]);
                if area.abs() < f32::EPSILON {
                    return None;
                }
                // edge(a, b, p) as a plane in p, scaled by 1 / area.
                let plane = |a: usize, b: usize| {
                    [
                        -(ys[b] - ys[a]) / area,
                        (xs[b] - xs[a]) / area,
                        ((ys[b] - ys[a]) * xs[a] - (xs[b] - xs[a]) * ys[a]) / area,
                    ]
                };
                let w0 = plane(1, 2);
                let w1 = plane(2, 0);
                let w2 = [-w0[0] - w1[0], -w0[1] - w1[1], 1.0 - w0[2] - w1[2]];
                // A vertex value as a plane: w0 * v0 + w1 * v1 + w2 * v2, with w2 = 1 - w0 - w1.
                let interpolate = |k: usize| {
                    let v = corners.map(|c| c[k]);
                    let mut p = [0, 1, 2].map(|j| w0[j] * (v[0] - v[2]) + w1[j] * (v[1] - v[2]));
                    p[2] += v[2];
                    p
                };
                Some(ScreenTriangle {
                    weights: [w0, w1, w2],
                    inv_depth: interpolate(2),
                    shade: interpolate(3),
                    nearest: corners[0][2].max(corners[1][2]).max(corners[2][2]),
                    color,
                    min_x,
                    max_x,
                    min_y,
                    max_y,
                })
            })
        })
        .collect();

    // Draw nearest triangles first so pixels hidden behind them fail the depth test before
    // any shading work.
    let mut screen_triangles = screen_triangles;
    screen_triangles.par_sort_unstable_by(|a, b| b.nearest.total_cmp(&a.nearest));

    // Bucket triangles into bands of rows so each band only visits triangles that touch it.
    let band_count = (height as usize).div_ceil(MESH_BAND_ROWS);
    let mut bands: Vec<Vec<u32>> = vec![Vec::new(); band_count];
    for (index, tri) in screen_triangles.iter().enumerate() {
        let first = tri.min_y as usize / MESH_BAND_ROWS;
        let last = tri.max_y as usize / MESH_BAND_ROWS;
        for band in &mut bands[first..=last] {
            band.push(index as u32);
        }
    }

    let width = width as usize;
    img_slice
        .par_chunks_mut(4 * width * MESH_BAND_ROWS)
        .zip(depth_buffer.par_chunks_mut(width * MESH_BAND_ROWS))
        .zip(bands.par_iter())
        .enumerate()
        .for_each(|(band, ((pixels, depths), triangles))| {
            let band_top = band * MESH_BAND_ROWS;
            let band_rows = depths.len() / width;
            for &index in triangles {
                let tri = &screen_triangles[index as usize];
                let first_row = (tri.min_y as usize).max(band_top);
                let last_row = (tri.max_y as usize).min(band_top + band_rows - 1);
                for y in first_row..=last_row {
                    let py = y as f32 + 0.5;
                    // Each weight is linear in x along the row; keep the x range where all are >= 0.
                    let mut left = tri.min_x as f32 + 0.5;
                    let mut right = tri.max_x as f32 + 0.5;
                    for w in &tri.weights {
                        let at_zero = w[1] * py + w[2];
                        if w[0] > f32::EPSILON {
                            left = left.max(-at_zero / w[0]);
                        } else if w[0] < -f32::EPSILON {
                            right = right.min(-at_zero / w[0]);
                        } else if at_zero < 0.0 {
                            right = f32::MIN;
                        }
                    }
                    if left > right {
                        continue;
                    }
                    let first_x = (left - 0.5).ceil() as usize;
                    let last_x = ((right - 0.5).floor() as usize).min(width - 1);
                    let row_start = (y - band_top) * width;
                    let inv_depth_row = tri.inv_depth[1] * py + tri.inv_depth[2];
                    let shade_row = tri.shade[1] * py + tri.shade[2];
                    for x in first_x..=last_x {
                        let px = x as f32 + 0.5;
                        // Hidden when depth >= d, i.e. inv_depth * d <= 1, so hidden pixels skip
                        // the division. An infinite d (nothing drawn there yet) never hides.
                        let inv_depth = tri.inv_depth[0] * px + inv_depth_row;
                        if inv_depth * depths[row_start + x] <= 1.0 {
                            continue;
                        }
                        let depth = 1.0 / inv_depth;
                        depths[row_start + x] = depth;
                        // Perspective-correct light level between the vertices.
                        let shade = (tri.shade[0] * px + shade_row) * depth;
                        let pixel = (row_start + x) * 4;
                        for k in 0..3 {
                            pixels[pixel + k] = (tri.color[k] * shade).clamp(0.0, 255.0) as u8;
                        }
                    }
                }
            }
        });
}

#[cfg(test)]
#[path = "tests/mesh.rs"]
mod mesh_tests;

#[cfg(test)]
#[path = "tests/collision.rs"]
mod collision_tests;

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

// Apply vertical movement and collisions; return [feet height, landed flag].
#[wasm_bindgen]
pub fn move_vertical(
    x: f32,               // Player's world x coordinate.
    y: f32,               // Player's world y coordinate.
    z: f32,               // Feet height in world units.
    distance: f32,        // Requested vertical travel in world units; positive means up.
    fall_distance: f32,   // Downward travel to apply when gravity is active.
    jetpack_active: bool, // Hover instead of applying gravity while airborne.
    world: &WasmWorld,    // Grid and solid models.
) -> Float32Array {
    let world = world.world();
    let collision = world.solids();
    let roof = roof_height_at(x, y, world.grid(), world.size());
    // A roof supports feet above it; underneath it, feet stand on the ground or a solid model.
    let support = if z >= 1.0 - COLLISION_EPSILON {
        roof
    } else {
        let feet = (x.floor() as i32, y.floor() as i32);
        solid_support_at(x, y, z, &collision.boxes_in(feet, feet))
    };
    let mut next_z = (z + distance).max(support);
    if !jetpack_active && distance > 0.0 {
        // D-key jumping reaches half a world unit above the ground or roof.
        next_z = next_z.min(support + 0.5);
    }
    if z < 1.0 - COLLISION_EPSILON && roof > 0.0 {
        // Eyes sit 0.5 units above feet; leave a gap below the ceiling at height 1.
        next_z = next_z.min(0.49);
    }
    let landed = distance < 0.0 && next_z <= support;
    if !jetpack_active || landed {
        // Gravity stops at the walking surface; hovering disables it until landing.
        next_z = (next_z - fall_distance).max(support);
    }
    Float32Array::from(&[next_z, if landed { 1.0 } else { 0.0 }][..])
}

fn roof_height_at(x: f32, y: f32, grid: &[u64], size: usize) -> f32 {
    let value = get_grid_value(x.floor() as i32, y.floor() as i32, size as i32, grid);
    // Ceiling bit 2 covers the roof; uncovered walls support feet on their own tops.
    if has_bit_set(value, 2) || inside_wall_footprint(value, x - x.floor(), y - y.floor()) {
        1.0
    } else {
        0.0
    }
}

// How far the player's centre keeps from the sides of walls, windows and solid models.
const PLAYER_RADIUS: f32 = 0.2;
const COLLISION_EPSILON: f32 = 0.000001;

// Walls and windows of one cell as world boxes up to wall-top height; doors stay passable.
fn wall_boxes(value: u64, cell_x: i32, cell_y: i32) -> impl Iterator<Item = CollisionBox> {
    let (x, y) = (cell_x as f32, cell_y as f32);
    wall_slots(value)
        .into_iter()
        .filter(|(_, door)| !door)
        .map(move |(r, _)| CollisionBox {
            min_x: x + r[0],
            min_y: y + r[1],
            max_x: x + r[2],
            max_y: y + r[3],
            top: 1.0,
        })
}

// Every wall, window and solid model box in the cells overlapping the area.
fn collision_boxes(
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
    grid: &[u64],
    size: i32,
    collision: &CollisionCells,
) -> Vec<CollisionBox> {
    let first = (min_x.floor() as i32, min_y.floor() as i32);
    let last = (max_x.floor() as i32, max_y.floor() as i32);
    // Solid models are stored once and listed by id, so one spanning cells comes back once.
    let mut boxes = collision.boxes_in(first, last);
    for cell_y in first.1..=last.1 {
        for cell_x in first.0..=last.0 {
            boxes.extend(wall_boxes(
                get_grid_value(cell_x, cell_y, size, grid),
                cell_x,
                cell_y,
            ));
        }
    }
    boxes
}

// Whether the step from (x, y) to (next_x, next_y) runs into the box widened by the player
// radius, with feet below its top. The whole step is tested, so long steps after slow
// frames cannot pass through. A player already inside a box may walk out of it.
fn step_enters_box(x: f32, y: f32, next_x: f32, next_y: f32, z: f32, b: &CollisionBox) -> bool {
    let (min_x, min_y) = (b.min_x - PLAYER_RADIUS, b.min_y - PLAYER_RADIUS);
    let (max_x, max_y) = (b.max_x + PLAYER_RADIUS, b.max_y + PLAYER_RADIUS);
    if z >= b.top - COLLISION_EPSILON || (x > min_x && x < max_x && y > min_y && y < max_y) {
        return false;
    }
    // Clip the step to the box one axis at a time; it enters when a part remains.
    let (mut enter, mut exit) = (0.0f32, 1.0f32);
    for (start, delta, low, high) in [(x, next_x - x, min_x, max_x), (y, next_y - y, min_y, max_y)]
    {
        if delta == 0.0 {
            if start <= low || start >= high {
                return false;
            }
        } else {
            let (a, b) = ((low - start) / delta, (high - start) / delta);
            enter = enter.max(a.min(b));
            exit = exit.min(a.max(b));
        }
    }
    enter < exit
}

// The highest solid top under the feet that they are at or above; 0 is the ground.
fn solid_support_at(x: f32, y: f32, z: f32, solids: &[CollisionBox]) -> f32 {
    solids
        .iter()
        .filter(|s| x >= s.min_x && x <= s.max_x && y >= s.min_y && y <= s.max_y)
        .filter(|s| z >= s.top - COLLISION_EPSILON)
        .fold(0.0, |top, s| top.max(s.top))
}

// Where a step of (dx, dy) ends: the full step, else its x or y part to slide along a box.
fn walk_step(
    x: f32,
    y: f32,
    dx: f32,
    dy: f32,
    z: f32,
    grid: &[u64],
    size: i32,
    collision: &CollisionCells,
) -> (f32, f32) {
    let boxes = collision_boxes(
        x.min(x + dx) - PLAYER_RADIUS,
        y.min(y + dy) - PLAYER_RADIUS,
        x.max(x + dx) + PLAYER_RADIUS,
        y.max(y + dy) + PLAYER_RADIUS,
        grid,
        size,
        collision,
    );
    let free = |next_x: f32, next_y: f32| {
        !boxes
            .iter()
            .any(|b| step_enters_box(x, y, next_x, next_y, z, b))
    };
    [(x + dx, y + dy), (x + dx, y), (x, y + dy)]
        .into_iter()
        .find(|&(next_x, next_y)| free(next_x, next_y))
        .unwrap_or((x, y))
}

// Move along the view direction unless a wall, window or solid model is in the way.
#[wasm_bindgen]
pub fn walk(
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    z: f32,            // Feet height; feet at or above a box's top pass over it.
    distance: f32,     // Requested travel in world units; negative walks back.
    world: &WasmWorld, // Grid and solid models.
) -> Float32Array {
    let world = world.world();
    let (x, y) = walk_step(
        x,
        y,
        dir_x * distance,
        dir_y * distance,
        z,
        world.grid(),
        world.size() as i32,
        world.solids(),
    );
    Float32Array::from(&[x, y][..])
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
