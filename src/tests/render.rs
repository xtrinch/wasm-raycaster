//! Optional native smoke render and timing harness for environments without a browser.
use super::*;
use std::time::Instant;

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
#[ignore = "manual render/performance check: cargo test --release render_views -- --ignored --nocapture"]
fn render_views() {
    render_fixture(false);
}

#[test]
fn exterior_stairs_are_hidden_by_interior_walls() {
    render_fixture(true);
}

fn render_fixture(check_occlusion: bool) {
    let source = include_str!("../../packages/app/src/vector-game/game/gridMap.ts");
    let packed = source
        .split("this.wallGrid = new BigUint64Array([")
        .nth(1)
        .unwrap()
        .split("].map")
        .next()
        .unwrap();
    let south_cell =
        |upper: bool| 8u64 | (1u64 << 10) | (2u64 << 16) | if upper { 1u64 << 18 } else { 0 };
    let packed = packed
        .replace(
            "stairCell(\"south\", \"lower\")",
            &format!("0x{:x}", south_cell(false)),
        )
        .replace(
            "stairCell(\"south\", \"upper\")",
            &format!("0x{:x}", south_cell(true)),
        );
    let mut map: Vec<u64> = packed
        .split("0x")
        .skip(1)
        .map(|s| {
            let hex: String = s.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            u64::from_str_radix(&hex, 16).unwrap()
        })
        .collect();
    let size = (map.len() as f32).sqrt() as usize;
    let mut geometry = Vec::new();
    for i in 0..12 {
        geometry.extend_from_slice(&[
            4.0,
            0.1 + i as f32 * 1.9 / 12.0,
            4.9,
            0.1 + (i + 1) as f32 * 1.9 / 12.0,
            0.0,
            (i + 1) as f32 / 12.0,
        ]);
    }
    let mut textures = WasmTextureMap::new();
    let mut metadata = WasmTextureMetaMap::new();
    for kind in 1..=14 {
        let mut texels = Vec::new();
        for y in 0..32 {
            for x in 0..32 {
                let checker = if (x / 4 + y / 4) % 2 == 0 { 180 } else { 120 };
                let color = match kind {
                    1 => [checker, checker / 2, checker / 3, 255],
                    2 => [checker, checker, checker, 255],
                    _ => [checker / 2, checker, checker / 2, 255],
                };
                texels.extend_from_slice(&color);
            }
        }
        textures.populate_from_array(kind, 0, &texels);
        metadata.populate_from_array(kind, 32, 32, 1);
    }
    let sprites = WasmStripePerCoordMap::new();
    let (width, height) = (960, 540);
    let mut sky = vec![0u8; 64 * 32 * 4];
    for pixel in sky.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[80, 130, 190, 255]);
    }
    let mut background = BackgroundImageWasm::new();
    background.set(sky.as_mut_ptr(), 64, 32, width, height);
    let mut pixels = vec![255u8; width as usize * height as usize * 4];
    let mut columns = vec![0.0; width as usize];
    let mut depth = vec![0.0; width as usize * height as usize];
    if check_occlusion {
        for y in [2.25, 2.5, 3.0, 4.0] {
            for z in [0, 150] {
                for pitch in [-120, 0, 120] {
                    let mut draw = |with_stairs: bool| {
                        let boxes = if with_stairs {
                            &geometry[..]
                        } else {
                            &geometry[12 * 6..]
                        };
                        render(
                            4.45,
                            y,
                            0.0,
                            -1.0,
                            -1.1,
                            0.0,
                            pitch,
                            z,
                            1.1,
                            pixels.as_mut_ptr(),
                            columns.as_mut_ptr(),
                            map.as_mut_ptr(),
                            size,
                            width,
                            height,
                            15,
                            40,
                            0,
                            &background,
                            &sprites,
                            &textures,
                            &metadata,
                            depth.as_mut_ptr(),
                            boxes.as_ptr(),
                            boxes.len() / 6,
                        );
                        pixels.clone()
                    };
                    let without = draw(false);
                    let with = draw(true);
                    let changed = without
                        .chunks_exact(4)
                        .zip(with.chunks_exact(4))
                        .filter(|(a, b)| a != b)
                        .count();
                    assert_eq!(
                        changed, 0,
                        "stairs leak through wall at y={y}, z={z}, pitch={pitch}"
                    );
                }
            }
        }
        return;
    }
    for (name, x, y, z, dx, dy, pitch) in [
        ("interior", 4.0, 4.0, 0, 0.0, -1.0, 0),
        ("stairs", 4.45, -2.2, 0, 0.0, 1.0, 0),
        ("climbing", 4.45, 0.5, 270, 0.0, 1.0, 0),
        ("roof", 4.45, 3.0, 540, 0.0, 1.0, 0),
        ("roof-edge", 4.45, 2.2, 540, 0.0, -1.0, 0),
        ("thin-wall-top", 9.5, 7.8, 540, 0.0, -1.0, -160),
    ] {
        let mut times = Vec::new();
        for _ in 0..12 {
            let start = Instant::now();
            render(
                x,
                y,
                dx,
                dy,
                dy * 1.1,
                -dx * 1.1,
                pitch,
                z,
                1.1,
                pixels.as_mut_ptr(),
                columns.as_mut_ptr(),
                map.as_mut_ptr(),
                size,
                width,
                height,
                15,
                40,
                0,
                &background,
                &sprites,
                &textures,
                &metadata,
                depth.as_mut_ptr(),
                geometry.as_ptr(),
                geometry.len() / 6,
            );
            times.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!("{name}: {:.2} ms median native render at {width}x{height} (synthetic textures, no billboards)",times[6]);
        if let Ok(directory) = std::env::var("RAYCASTER_CAPTURE") {
            let mut image = format!("P6\n{width} {height}\n255\n").into_bytes();
            for pixel in pixels.chunks_exact(4) {
                image.extend_from_slice(&pixel[..3]);
            }
            std::fs::write(
                std::path::Path::new(&directory).join(format!("{name}.ppm")),
                image,
            )
            .unwrap();
        }
    }
}
