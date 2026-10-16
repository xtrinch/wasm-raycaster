use super::*;
#[test]
fn wall_top_coverage_matches_thickness_offsets_and_orientation() {
    // Horizontal segment: x=.2..8, y=.4..5. Vertical swaps the axes.
    let segment = 0x2614u64 << 16;
    let horizontal = segment | (1 << 12) | (1 << 6) | 9;
    let vertical = segment | (1 << 12) | 9;
    assert!(inside_wall(horizontal, 0.5, 0.45));
    assert!(!inside_wall(horizontal, 0.1, 0.45));
    assert!(!inside_wall(horizontal, 0.5, 0.55));
    assert!(inside_wall(vertical, 0.45, 0.5));
    assert!(!inside_wall(vertical, 0.55, 0.5));
}

#[test]
fn looking_down_draws_wall_top_with_visible_thickness() {
    let texels = [180, 180, 180, 255];
    let texture = || Texture {
        data: &texels,
        width: 1,
        height: 1,
    };
    let plane = FloorPlane {
        height: 1.0,
        facing: Facing::Up,
        cells: 2,
        texture: texture(),
        alternate: None,
        wall_tops: Some(texture()),
    };
    let mut p = camera(64);
    p.x = 0.5;
    p.y = -0.25;
    p.pitch = -8;
    let render = |thickness: u64| {
        let grid = [((0x0a04 | (thickness << 4)) << 16) | (1 << 12) | (1 << 6) | 9];
        let mut pixels = vec![0; 64 * 64 * 4];
        let mut depth = vec![0.0; 64 * 64];
        draw(
            &p,
            &mut pixels,
            &mut depth,
            std::slice::from_ref(&plane),
            &grid,
            1,
            64,
            64,
            15,
            0,
        );
        (0..64).filter(|y| depth[y * 64 + 32].is_finite()).count()
    };
    let thin = render(1);
    let wider = render(3);
    assert!(
        thin >= 3,
        "a .1-unit wall top must cover more than one pixel row: {thin}"
    );
    assert!(
        wider > thin,
        "projected wall width must follow its grid thickness"
    );
}
fn camera(z: i32) -> Position {
    Position {
        x: 4.0,
        y: 4.0,
        map_x: 4,
        map_y: 4,
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
fn floors_at_multiple_heights_use_the_same_renderer_and_nearest_wins() {
    let red = [200, 0, 0, 255];
    let blue = [0, 0, 200, 255];
    let texture = |data| Texture {
        data,
        width: 1,
        height: 1,
    };
    let low = FloorPlane {
        height: 0.0,
        facing: Facing::Up,
        cells: 2,
        texture: texture(&red),
        alternate: None,
        wall_tops: None,
    };
    let upper = FloorPlane {
        height: 2.0,
        facing: Facing::Up,
        cells: 2,
        texture: texture(&blue),
        alternate: None,
        wall_tops: None,
    };
    let render = |planes: &[FloorPlane<'_>]| {
        let mut pixels = vec![0; 64 * 64 * 4];
        let mut depth = vec![0.0; 64 * 64];
        draw(
            &camera(128),
            &mut pixels,
            &mut depth,
            planes,
            &vec![2; 32 * 32],
            32,
            64,
            64,
            15,
            0,
        );
        (pixels, depth)
    };
    let a = render(&[low, upper]);
    let index = 60 * 64 + 32;
    assert_eq!(a.0[index * 4], 0);
    assert!(a.0[index * 4 + 2] > 0);
    let expected = 0.5 * 32.0 / 28.5;
    assert!((a.1[index] - expected).abs() < 1e-5);
    let b = render(&[
        FloorPlane {
            height: 2.0,
            facing: Facing::Up,
            cells: 2,
            texture: texture(&blue),
            alternate: None,
            wall_tops: None,
        },
        FloorPlane {
            height: 0.0,
            facing: Facing::Up,
            cells: 2,
            texture: texture(&red),
            alternate: None,
            wall_tops: None,
        },
    ]);
    assert_eq!(a, b);
}
#[test]
fn ceiling_and_floor_share_height_with_opposite_visible_sides() {
    let texels = [180, 180, 180, 255];
    for (z, facing, row) in [(0, Facing::Down, 10), (64, Facing::Up, 54)] {
        let mut pixels = vec![0; 64 * 64 * 4];
        let mut depth = vec![0.0; 64 * 64];
        let plane = FloorPlane {
            height: 1.0,
            facing,
            cells: 2,
            texture: Texture {
                data: &texels,
                width: 1,
                height: 1,
            },
            alternate: None,
            wall_tops: None,
        };
        draw(
            &camera(z),
            &mut pixels,
            &mut depth,
            &[plane],
            &vec![2; 32 * 32],
            32,
            64,
            64,
            15,
            0,
        );
        assert!(depth[row * 64 + 32].is_finite());
        assert!(depth[(63 - row) * 64 + 32].is_infinite());
    }
}
#[test]
fn grid_gaps_remain_open_at_any_plane_height() {
    let texels = [180, 180, 180, 255];
    let plane = FloorPlane {
        height: 1.0,
        facing: Facing::Up,
        cells: 2,
        texture: Texture {
            data: &texels,
            width: 1,
            height: 1,
        },
        alternate: None,
        wall_tops: None,
    };
    let mut pixels = vec![73; 64 * 64 * 4];
    let mut depth = vec![0.0; 64 * 64];
    draw(
        &camera(64),
        &mut pixels,
        &mut depth,
        &[plane],
        &vec![8; 32 * 32],
        32,
        64,
        64,
        15,
        0,
    );
    assert!(pixels.iter().all(|v| *v == 73));
    assert!(depth.iter().all(|v| v.is_infinite()));
}
