//! Horizontal planes share one row-based renderer, regardless of story height.
use crate::helpers::{
    get_bits, get_grid_value, has_bit_set, Position, Texture, FIXED_ONE, FIXED_SHIFT,
};
use rayon::prelude::*;

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

pub(super) fn inside_wall(value: u64, x: f32, y: f32) -> bool {
    if value == 1 {
        return true;
    }
    for index in 0..get_bits(value, 12).min(3) {
        let shift = 16 + index * 16;
        let primary = (get_bits(value, shift) % 11) as f32 / 10.0;
        let thickness = (get_bits(value, shift + 4) % 11) as f32 / 10.0;
        let length = (get_bits(value, shift + 8) % 11) as f32 / 10.0;
        let secondary = (get_bits(value, shift + 12) % 11) as f32 / 10.0;
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

pub fn draw(
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
    let eye = 0.5 + position.z as f32 / height as f32;
    let middle = height as f32 * 0.5 + position.pitch as f32;
    pixels
        .par_chunks_mut(width as usize * 4)
        .zip(depth.par_chunks_mut(width as usize))
        .enumerate()
        .for_each(|(y, (row, zrow))| {
            zrow.fill(f32::INFINITY);
            let screen_offset = y as f32 + 0.5 - middle;
            // Nearer planes are normally declared last; depth also makes order safe.
            for plane in planes.iter().rev() {
                let visible = match plane.facing {
                    Facing::Up => eye > plane.height,
                    Facing::Down => eye < plane.height,
                };
                if !visible {
                    continue;
                }
                let distance = (eye - plane.height) * width as f32 * 0.5 / screen_offset;
                let ray_distance = distance / position.plane_y_initial;
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
                    if distance < *z {
                        let texture = if value & plane.cells != 0 {
                            Some(match &plane.alternate {
                                Some((flag, texture)) if value & flag != 0 => texture,
                                _ => &plane.texture,
                            })
                        } else {
                            plane.wall_tops.as_ref().filter(|_| {
                                inside_wall(
                                    value,
                                    (world_x & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                                    (world_y & (FIXED_ONE as i64 - 1)) as f32 / FIXED_ONE as f32,
                                )
                            })
                        };
                        if let Some(texture) = texture {
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

#[cfg(test)]
#[path = "tests/floors.rs"]
mod tests;
