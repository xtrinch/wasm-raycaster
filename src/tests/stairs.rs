use super::*;

fn camera(z: i32) -> Position {
    Position {
        x: 0.5,
        y: -1.0,
        map_x: 0,
        map_y: -1,
        dir_x: 0.0,
        dir_y: 1.0,
        plane_x: 1.1,
        plane_y: 0.0,
        pitch: 0,
        z,
        plane_y_initial: 1.1,
    }
}

#[test]
fn step_projects_at_existing_wall_top() {
    let p = camera(0);
    let vertices = [
        Vertex {
            x: 0.0,
            y: 1.0,
            z: 1.0,
        },
        Vertex {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        },
        Vertex {
            x: 1.0,
            y: 2.0,
            z: 1.0,
        },
        Vertex {
            x: 0.0,
            y: 2.0,
            z: 1.0,
        },
    ];
    let face = project_face(&p, vertices, 2, 1.0, 1.0, 320, 200).unwrap();
    let wall =
        crate::translate_coordinate_to_camera(&p, -0.5, 2.0, 1.0, 320, 200, 200.0 / 320.0, 1.1);
    assert!((face.polygon[0].1 - wall.screen_y_ceiling as f32).abs() < 1.0);
}

#[test]
fn near_clipping_and_step_depth_are_finite() {
    let p = camera(200);
    let mut pixels = vec![0; 64 * 64 * 4];
    let mut depth = vec![f32::INFINITY; 64 * 64];
    let texels = [180, 160, 140, 255];
    let texture = || Texture {
        data: &texels,
        width: 1,
        height: 1,
    };
    // Camera lies inside the step footprint; polygon crosses the near plane.
    draw(
        &p,
        &mut pixels,
        &mut depth,
        &[0.0, -2.0, 1.0, 3.0, 0.0, 1.0],
        64,
        64,
        texture(),
        15,
        0,
    );
    assert!(depth.iter().any(|d| d.is_finite() && *d > 0.0));
    assert!(depth.iter().all(|d| !d.is_nan() && *d > 0.0));
    assert!(depth[..32 * 64].iter().all(|d| d.is_infinite()));
}

#[test]
fn nearer_surface_wins_independent_of_box_order() {
    let p = camera(0);
    let texels = [180, 160, 140, 255];
    let texture = || Texture {
        data: &texels,
        width: 1,
        height: 1,
    };
    let a = [0.0, 0.0, 1.0, 0.5, 0.0, 0.25];
    let b = [0.0, 1.0, 1.0, 1.5, 0.0, 0.5];
    let render = |boxes: &[f32]| {
        let mut pixels = vec![0; 64 * 64 * 4];
        let mut depth = vec![f32::INFINITY; 64 * 64];
        draw(&p, &mut pixels, &mut depth, boxes, 64, 64, texture(), 15, 0);
        (pixels, depth)
    };
    assert_eq!(render(&[a, b].concat()), render(&[b, a].concat()));
}

#[test]
fn vertical_textures_run_from_top_to_bottom() {
    let p = camera(0);
    let texels = [20, 0, 0, 255, 220, 0, 0, 255];
    let texture = || Texture {
        data: &texels,
        width: 1,
        height: 2,
    };
    let mut pixels = vec![0; 64 * 64 * 4];
    let mut depth = vec![f32::INFINITY; 64 * 64];
    draw(
        &p,
        &mut pixels,
        &mut depth,
        &[0.0, 0.0, 1.0, 1.0, 0.0, 1.0],
        64,
        64,
        texture(),
        15,
        0,
    );
    let upper = (24 * 64 + 32) * 4;
    let lower = (40 * 64 + 32) * 4;
    assert!(depth[24 * 64 + 32].is_finite() && depth[40 * 64 + 32].is_finite());
    assert!(
        pixels[upper] < pixels[lower],
        "top texture row must appear above bottom row"
    );
}
