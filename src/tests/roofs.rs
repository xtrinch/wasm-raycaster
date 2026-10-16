use super::*;

#[test]
fn pitched_background_replaces_previous_frame_including_panorama_edges() {
    let (width, height) = (16, 16);
    let mut sky = Vec::new();
    for y in 0..32 {
        for x in 0..64 {
            sky.extend_from_slice(&[x as u8, y as u8, 80, 255]);
        }
    }
    let mut background = BackgroundImageWasm::new();
    background.set(sky.as_mut_ptr(), 64, 32, width, height);
    for pitch in [-200, -5, 0, 5, 200] {
        for (dir_x, dir_y) in [(0.0, 1.0), (1.0, 0.0), (-1.0, 0.0)] {
            let p = Position {
                x: 4.45,
                y: 2.1,
                map_x: 4,
                map_y: 2,
                dir_x,
                dir_y,
                plane_x: dir_y * 1.1,
                plane_y: -dir_x * 1.1,
                pitch,
                z: height,
                plane_y_initial: 1.1,
            };
            let mut fresh = vec![0; width as usize * height as usize * 4];
            let mut previous_frame = vec![231; fresh.len()];
            draw_background_image_prescaled(&p, &background, &mut fresh, width, height);
            draw_background_image_prescaled(&p, &background, &mut previous_frame, width, height);
            assert_eq!(
                previous_frame, fresh,
                "stale pixels at pitch {pitch}, direction ({dir_x},{dir_y})"
            );
            assert!(fresh.chunks_exact(4).all(|pixel| pixel[3] == 255));
        }
    }
}

#[test]
fn roof_support_matches_full_cells_and_thin_wall_footprints() {
    let thin = (0x0a14u64 << 16) | (1 << 12) | (1 << 6) | 9;
    let grid = [6, 8, thin, 1];
    assert_eq!(roof_height_at(0.5, 0.5, &grid, 2), 1.0);
    assert_eq!(roof_height_at(1.5, 0.5, &grid, 2), 0.0);
    assert_eq!(roof_height_at(0.5, 1.45, &grid, 2), 1.0);
    assert_eq!(roof_height_at(0.5, 1.7, &grid, 2), 0.0);
    assert_eq!(roof_height_at(1.5, 1.5, &grid, 2), 1.0);
    assert_eq!(roof_height_at(-0.5, 0.5, &grid, 2), 0.0);
}
