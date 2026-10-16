//! Static axis-aligned stair boxes, clipped and scan converted before shading.
//! Work scales with covered pixels, rather than casting extra rays per pixel.
use crate::helpers::{Position, Texture};
use rayon::prelude::*;

#[derive(Clone, Copy)]
struct Vertex {
    x: f32,
    y: f32,
    z: f32,
}
struct Face {
    polygon: Vec<(f32, f32)>,
    axis: usize,
    coordinate: f32,
    shade: f32,
}

fn forward(p: &Position, v: Vertex) -> f32 {
    (v.x - p.x) * p.dir_x + (v.y - p.y) * p.dir_y
}

fn project_face(
    p: &Position,
    vertices: [Vertex; 4],
    axis: usize,
    coordinate: f32,
    shade: f32,
    width: i32,
    height: i32,
) -> Option<Face> {
    // Sutherland-Hodgman near clipping also handles standing on a roof/stair.
    let mut clipped = Vec::with_capacity(5);
    for i in 0..4 {
        let a = vertices[i];
        let b = vertices[(i + 1) % 4];
        let da = forward(p, a);
        let db = forward(p, b);
        if da >= 0.02 {
            clipped.push(a);
        }
        if (da >= 0.02) != (db >= 0.02) {
            let t = (0.02 - da) / (db - da);
            clipped.push(Vertex {
                x: a.x + t * (b.x - a.x),
                y: a.y + t * (b.y - a.y),
                z: a.z + t * (b.z - a.z),
            });
        }
    }
    if clipped.len() < 3 {
        return None;
    }
    let eye = 0.5 + p.z as f32 / height as f32;
    let polygon: Vec<_> = clipped
        .iter()
        .map(|v| {
            let depth = forward(p, *v) * p.plane_y_initial;
            let lateral = p.dir_y * (v.x - p.x) - p.dir_x * (v.y - p.y);
            (
                width as f32 * 0.5 * (1.0 + lateral / depth),
                height as f32 * 0.5 + p.pitch as f32 + (eye - v.z) * width as f32 * 0.5 / depth,
            )
        })
        .collect();
    if polygon.iter().all(|v| v.0 < 0.0)
        || polygon.iter().all(|v| v.0 >= width as f32)
        || polygon.iter().all(|v| v.1 < 0.0)
        || polygon.iter().all(|v| v.1 >= height as f32)
    {
        return None;
    }
    Some(Face {
        polygon,
        axis,
        coordinate,
        shade,
    })
}

pub fn draw(
    p: &Position,
    pixels: &mut [u8],
    depth: &mut [f32],
    boxes: &[f32],
    width: i32,
    height: i32,
    texture: Texture<'_>,
    light_range: i32,
    map_light: i32,
) {
    let mut faces = Vec::with_capacity(boxes.len());
    let eye = 0.5 + p.z as f32 / height as f32;
    for b in boxes.chunks_exact(6) {
        let (x0, y0, x1, y1, z0, z1) = (b[0], b[1], b[2], b[3], b[4], b[5]);
        let v = |x, y, z| Vertex { x, y, z };
        let mut add = |vertices, axis, coordinate, shade| {
            if let Some(face) = project_face(p, vertices, axis, coordinate, shade, width, height) {
                faces.push(face);
            }
        };
        if eye > z1 {
            add(
                [v(x0, y0, z1), v(x1, y0, z1), v(x1, y1, z1), v(x0, y1, z1)],
                2,
                z1,
                1.0,
            );
        }
        if z0 == z1 {
            continue;
        }
        if p.x < x0 {
            add(
                [v(x0, y0, z0), v(x0, y1, z0), v(x0, y1, z1), v(x0, y0, z1)],
                0,
                x0,
                0.75,
            );
        }
        if p.x > x1 {
            add(
                [v(x1, y0, z0), v(x1, y1, z0), v(x1, y1, z1), v(x1, y0, z1)],
                0,
                x1,
                0.75,
            );
        }
        if p.y < y0 {
            add(
                [v(x0, y0, z0), v(x1, y0, z0), v(x1, y0, z1), v(x0, y0, z1)],
                1,
                y0,
                0.85,
            );
        }
        if p.y > y1 {
            add(
                [v(x0, y1, z0), v(x1, y1, z0), v(x1, y1, z1), v(x0, y1, z1)],
                1,
                y1,
                0.85,
            );
        }
    }
    pixels
        .par_chunks_mut(width as usize * 4)
        .zip(depth.par_chunks_mut(width as usize))
        .enumerate()
        .for_each(|(y, (row, zrow))| {
            let screen_y = y as f32 + 0.5;
            let ray_z = (height as f32 * 0.5 + p.pitch as f32 - screen_y) * 2.0 * p.plane_y_initial
                / width as f32;
            for face in &faces {
                let mut left = f32::INFINITY;
                let mut right = f32::NEG_INFINITY;
                for i in 0..face.polygon.len() {
                    let a = face.polygon[i];
                    let b = face.polygon[(i + 1) % face.polygon.len()];
                    if (a.1 <= screen_y && b.1 > screen_y) || (b.1 <= screen_y && a.1 > screen_y) {
                        let x = a.0 + (screen_y - a.1) * (b.0 - a.0) / (b.1 - a.1);
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
                if left >= right {
                    continue;
                }
                let start = (left - 0.5).ceil().max(0.0) as usize;
                let end = (right - 0.5).ceil().clamp(0.0, width as f32) as usize;
                for x in start..end {
                    let camera_x = 2.0 * (x as f32 + 0.5) / width as f32 - 1.0;
                    let ray_x = p.dir_x + p.plane_x * camera_x;
                    let ray_y = p.dir_y + p.plane_y * camera_x;
                    let t = match face.axis {
                        0 => (face.coordinate - p.x) / ray_x,
                        1 => (face.coordinate - p.y) / ray_y,
                        _ => (face.coordinate - eye) / ray_z,
                    };
                    let distance = t * p.plane_y_initial;
                    if t <= 0.0 || !distance.is_finite() || distance >= zrow[x] {
                        continue;
                    }
                    let wx = p.x + t * ray_x;
                    let wy = p.y + t * ray_y;
                    let wz = eye + t * ray_z;
                    let (u, v) = match face.axis {
                        0 => (wy, 1.0 - wz),
                        1 => (wx, 1.0 - wz),
                        _ => (wx, wy),
                    };
                    let tx = (u.rem_euclid(1.0) * texture.width as f32) as usize
                        % texture.width as usize;
                    let ty = (v.rem_euclid(1.0) * texture.height as f32) as usize
                        % texture.height as usize;
                    let idx = (ty * texture.width as usize + tx) * 4;
                    let light = (1.0 - distance / light_range as f32 + map_light as f32)
                        .clamp(0.125, 1.0)
                        * face.shade;
                    for c in 0..3 {
                        row[x * 4 + c] = (texture.data[idx + c] as f32 * light) as u8;
                    }
                    zrow[x] = distance;
                }
            }
        });
}

#[cfg(test)]
#[path = "tests/stairs.rs"]
mod tests;
