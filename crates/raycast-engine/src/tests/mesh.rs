use super::*;
use crate::world_helpers::{Mesh, Model};

const WIDTH: i32 = 64;
const HEIGHT: i32 = 48;

// Standing at (0.5, 5) on the ground, looking north (-y) like the game's start.
fn camera() -> Position {
    Position {
        x: 0.5,
        y: 5.0,
        map_x: 0,
        map_y: 5,
        dir_x: 0.0,
        dir_y: -1.0,
        plane_x: -1.1,
        plane_y: 0.0,
        pitch: 0,
        z: 0.0,
        plane_y_initial: 1.1,
    }
}

// One triangle in draw_meshes' layout, with a single normal for all three vertices.
fn triangle(vertices: [[f32; 3]; 3], normal: [f32; 3], color: [f32; 3]) -> Vec<f32> {
    let mut data = Vec::with_capacity(MESH_TRIANGLE_STRIDE);
    for vertex in vertices {
        data.extend_from_slice(&vertex);
        data.extend_from_slice(&normal);
    }
    data.extend_from_slice(&color);
    data
}

// A one-unit-tall vertical quad facing the camera at the given y, spanning x 0..1.
fn colored_quad(y: f32, color: [f32; 3]) -> Vec<f32> {
    let normal = [0.0, 1.0, 0.0];
    [
        triangle([[0.0, y, 0.0], [1.0, y, 0.0], [1.0, y, 1.0]], normal, color),
        triangle([[0.0, y, 0.0], [1.0, y, 1.0], [0.0, y, 1.0]], normal, color),
    ]
    .concat()
}

fn wall_quad(y: f32) -> Vec<f32> {
    colored_quad(y, [1.0, 0.0, 0.0])
}

struct Frame {
    image: Vec<u8>,
    depth: Vec<f32>, // floor, ceiling, roof and mesh depth per pixel
}

fn frame() -> Frame {
    let pixels = (WIDTH * HEIGHT) as usize;
    Frame {
        image: vec![0; pixels * 4],
        depth: vec![f32::INFINITY; pixels],
    }
}

fn draw(frame: &mut Frame, triangles: &[f32]) {
    draw_meshes(
        &camera(),
        &mut frame.image,
        WIDTH,
        HEIGHT,
        &[triangles],
        &mut frame.depth,
        1000,
        0,
    );
}

fn drawn(frame: &Frame, x: i32, y: i32) -> bool {
    frame.image[((y * WIDTH + x) * 4) as usize] > 0
}

#[test]
fn quad_matches_sprite_projection() {
    let mut frame = frame();
    draw(&mut frame, &wall_quad(3.0));

    let position = camera();
    let inv_det = (position.plane_x * position.dir_y - position.dir_x * position.plane_y).abs();
    let sprite = translate_coordinate_to_camera(
        &position,
        0.0,
        -2.0,
        1.0,
        WIDTH,
        HEIGHT,
        HEIGHT as f32 / WIDTH as f32,
        inv_det,
    );

    let column = WIDTH / 2;
    let rows: Vec<i32> = (0..HEIGHT).filter(|&y| drawn(&frame, column, y)).collect();
    let (top, bottom) = (rows[0], *rows.last().unwrap());
    assert!(
        (top - sprite.screen_y_ceiling).abs() <= 1,
        "top {top} vs sprite {}",
        sprite.screen_y_ceiling
    );
    assert!(
        (bottom + 1 - (sprite.screen_y_ceiling + sprite.full_height)).abs() <= 1,
        "bottom {bottom} vs sprite {}",
        sprite.screen_y_ceiling + sprite.full_height
    );
    let depth = frame.depth[(HEIGHT / 2 * WIDTH + column) as usize];
    assert!(
        (depth - sprite.distance).abs() < 1e-3,
        "depth {depth} vs sprite {}",
        sprite.distance
    );
}

#[test]
fn nearer_floors_hide_the_mesh() {
    let mut frame = frame();
    let floor_pixel = ((HEIGHT / 2 + 1) * WIDTH + WIDTH / 2) as usize;
    frame.depth[floor_pixel] = 1.0;
    draw(&mut frame, &wall_quad(3.0));

    assert!(!drawn(&frame, WIDTH / 2, HEIGHT / 2 + 1));
    assert!(drawn(&frame, WIDTH / 2, HEIGHT / 2));
}

// Draws a red mesh quad at mesh_y, then a one-texel wall or window stripe at y 3 in the
// centre column with the given texture alpha; returns the centre pixel's rgb.
fn mesh_then_stripe(mesh_y: f32, texture: TextureType, alpha: u8) -> [u8; 3] {
    let texture = texture as i32;
    let mut frame = frame();
    draw(&mut frame, &wall_quad(mesh_y));

    let mut textures = TextureMap::new();
    textures.insert(texture, 0, &[255, 255, 255, alpha]);
    let mut texture_meta = TextureMetaMap::new();
    texture_meta.insert(texture, 1, 1, 1);
    let mut stripes: SmallVec<[Sprite; 1024]> = SmallVec::new();
    stripes.push(Sprite {
        x: 0.5,
        y: 3.0,
        fract: 0.5,
        dx: 0.0,
        dy: 0.0,
        angle: 0,
        height: 100,
        r#type: texture,
        column: (WIDTH / 2) as u32,
        distance: 2.0,
        distance_fixed: 0,
    });
    let mut column_depth = vec![f32::INFINITY; WIDTH as usize];
    draw_sprites(
        &camera(),
        &mut frame.image,
        WIDTH,
        HEIGHT,
        &mut column_depth,
        1000,
        0,
        &textures,
        &texture_meta,
        &mut stripes,
        &frame.depth,
    );

    let pixel = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    [
        frame.image[pixel],
        frame.image[pixel + 1],
        frame.image[pixel + 2],
    ]
}

#[test]
fn walls_and_sprites_composite_with_meshes_by_depth() {
    // Window glass in front of the mesh blends over it: white glass plus red showing through.
    let [r, g, _] = mesh_then_stripe(2.0, TextureType::WINDOW, 128);
    assert!(
        g > 100 && g < 160 && r > g + 40,
        "glass did not blend over mesh: {r} {g}"
    );

    // An opaque wall in front of the mesh covers it.
    let [r, g, b] = mesh_then_stripe(2.0, TextureType::WALL, 255);
    assert!(
        r > 240 && g > 240 && b > 240,
        "wall did not cover mesh: {r} {g} {b}"
    );

    // A mesh in front of a window or wall hides it.
    for texture in [TextureType::WINDOW, TextureType::WALL] {
        let [r, g, _] = mesh_then_stripe(4.0, texture, 128);
        assert!(r > 0 && g == 0, "stripe drew over a nearer mesh: {r} {g}");
    }
}

#[test]
fn nearer_triangle_wins_regardless_of_order() {
    let far = colored_quad(3.0, [0.0, 1.0, 0.0]);
    let near = wall_quad(4.0);

    for triangles in [[near.clone(), far.clone()].concat(), [far, near].concat()] {
        let mut frame = frame();
        draw(&mut frame, &triangles);
        let pixel = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
        assert!(
            frame.image[pixel] > 0 && frame.image[pixel + 1] == 0,
            "far quad drew over near quad"
        );
    }
}

#[test]
fn triangles_crossing_behind_the_camera_are_clipped() {
    let mut frame = frame();
    // A floor-level triangle from behind the player to far in front of them.
    let triangles = triangle(
        [[0.0, 8.0, 0.2], [1.0, 8.0, 0.2], [0.5, 1.0, 0.2]],
        [0.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
    );
    draw(&mut frame, &triangles);

    assert!((0..HEIGHT).any(|y| drawn(&frame, WIDTH / 2, y)));
    assert!(frame.depth.iter().all(|d| *d >= MESH_NEAR_DEPTH * 0.99));
}

// A model whose unrotated footprint is centred on x/y with the given half sizes; its mesh has
// bounds only, no triangles.
fn place_model(world: &mut World, x: f64, y: f64, half_x: f64, half_y: f64) -> u32 {
    let mesh = world.add_mesh(Mesh {
        positions: vec![-half_x, 0.0, -half_y, half_x, 1.0, half_y],
        triangles: vec![],
        normals: vec![],
        colors: vec![],
    });
    world
        .add_model(Model {
            mesh,
            x,
            y,
            z: 0.0,
            scale: 1.0,
            height: None,
            angle: 0.0,
            color: [0.8; 3],
            solid: false,
        })
        .unwrap()
}

#[test]
fn model_footprints_register_every_cell_they_touch() {
    let mut world = World::new(9);
    // Footprint x 1.5..3, y 2.5..3.2.
    let id = place_model(&mut world, 2.25, 2.85, 0.75, 0.35);

    for cell in [(1, 2), (1, 3), (2, 2), (2, 3)] {
        assert_eq!(world.models_at(&cell), &[id], "cell {cell:?}");
    }
    // x ends exactly on the line x = 3, so column 3 is not reached.
    assert!(world.models_at(&(3, 2)).is_empty() && world.models_at(&(3, 3)).is_empty());
    assert!(world.model_triangles(id).is_some());
    assert!(world.model_triangles(id + 1).is_none());
    assert!(world.occupied(&(2, 3)) && !world.occupied(&(3, 3)));
}

#[test]
fn only_models_in_cells_the_rays_reach_are_found() {
    // A 9 by 9 grid with a thin north wall along y = 2 across every column.
    let size = 9;
    let mut grid = vec![0u64; size * size];
    for x in 0..size {
        grid[2 * size + x] = 0x0A10_1041;
    }
    let mut world = World::new(size);
    let in_front = place_model(&mut world, 4.5, 3.5, 0.3, 0.3); // x 4.2..4.8, y 3.2..3.8
    place_model(&mut world, 4.5, 1.5, 0.3, 0.3); // behind the wall
    place_model(&mut world, 4.5, 5.5, 0.3, 0.3); // behind the camera
    let spanning = place_model(&mut world, 4.5, 3.45, 1.0, 0.45); // x 3.5..5.5: three reached cells

    let mut position = camera();
    position.x = 4.5;
    position.y = 4.5;
    position.map_x = 4;
    position.map_y = 4;
    let width = 40;
    let mut column_depth = vec![0.0; width as usize];
    let mut sprites: SmallVec<[Sprite; 1024]> = SmallVec::new();
    let mut found = Vec::new();
    draw_walls_raycast(
        &position,
        &mut column_depth,
        &grid,
        size,
        width,
        20,
        64,
        &world,
        &mut sprites,
        &mut found,
    );

    assert_eq!(found, vec![in_front, spanning]);
}
