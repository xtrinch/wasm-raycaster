// Renders fixed scenes through render() and compares each frame with a reference PNG in
// tests/snapshots/. Textures are procedural, so the references do not depend on game art.
//
// After an intended rendering change, regenerate the references and look at them:
//   UPDATE_SNAPSHOTS=1 cargo test snapshot
// A mismatch writes the rendered frame next to the reference as <name>.actual.png.
use super::*;
use std::fs::File;
use std::path::PathBuf;

const WIDTH: i32 = 160;
const HEIGHT: i32 = 120;
const LIGHT_RANGE: i32 = 15; // camera.ts lightRange
const RANGE: i8 = 40; // camera.ts range

// Looking north (-y) from (x, y), like the game's spawn, with feet at height z.
fn camera(x: f32, y: f32, z: f32, pitch: i32) -> Position {
    Position {
        x,
        y,
        map_x: x.floor() as i32,
        map_y: y.floor() as i32,
        dir_x: 0.0,
        dir_y: -1.0,
        plane_x: -1.1,
        plane_y: 0.0,
        pitch,
        z,
        plane_y_initial: 1.1,
    }
}

// RGBA texels, row-major, from a function of the texel position.
fn texture(width: i32, height: i32, texel: impl Fn(i32, i32) -> [u8; 4]) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| texel(x, y))
        .collect()
}

fn checker(width: i32, height: i32, square: i32, a: [u8; 3], b: [u8; 3]) -> Vec<u8> {
    texture(width, height, |x, y| {
        let [r, g, b] = if (x / square + y / square) % 2 == 0 {
            a
        } else {
            b
        };
        [r, g, b, 255]
    })
}

struct Assets {
    textures: TextureMap,
    meta: TextureMetaMap,
    background: Background,
}

fn assets() -> Assets {
    let mut textures = TextureMap::new();
    let mut meta = TextureMetaMap::new();
    let mut add = |kind: TextureType, width: i32, height: i32, data: Vec<u8>| {
        let key = kind as i32;
        textures.insert(key, 0, &data);
        meta.insert(key, width, height, 1);
    };
    add(
        TextureType::WALL,
        16,
        16,
        checker(16, 16, 4, [170, 170, 160], [120, 120, 110]),
    );
    add(
        TextureType::CEILING,
        16,
        16,
        checker(16, 16, 8, [90, 110, 150], [60, 80, 120]),
    );
    add(
        TextureType::FLOOR,
        16,
        16,
        checker(16, 16, 8, [220, 220, 220], [40, 40, 40]),
    );
    add(
        TextureType::ROAD,
        16,
        16,
        checker(16, 16, 2, [100, 90, 80], [80, 70, 60]),
    );
    // Red roof tiles: horizontal stripes.
    add(
        TextureType::ROOF,
        16,
        16,
        texture(16, 16, |_, y| {
            if y % 4 < 2 {
                [150, 50, 40, 255]
            } else {
                [110, 30, 30, 255]
            }
        }),
    );
    // A brown door with a lighter frame.
    add(
        TextureType::DOOR,
        16,
        32,
        texture(16, 32, |x, y| {
            if x < 2 || x > 13 || y < 2 {
                [200, 160, 110, 255]
            } else {
                [110, 60, 30, 255]
            }
        }),
    );
    // An opaque white frame around translucent blue glass.
    add(
        TextureType::WINDOW,
        16,
        16,
        texture(16, 16, |x, y| {
            if x < 2 || x > 13 || y < 2 || y > 13 {
                [240, 240, 240, 255]
            } else {
                [120, 170, 220, 96]
            }
        }),
    );
    // A green cone on a transparent background.
    add(
        TextureType::TREE_CONE,
        16,
        32,
        texture(16, 32, |x, y| {
            if (x * 2 - 15).abs() <= y / 2 {
                [40, 150 + (y * 2) as u8, 60, 255]
            } else {
                [0, 0, 0, 0]
            }
        }),
    );

    // A sky that darkens towards the top, with vertical stripes to show the heading.
    let sky = texture(64, 32, |x, y| {
        let stripe = if x % 16 < 2 { 40 } else { 0 };
        [100 + y as u8 * 3, 140 + y as u8 * 2 + stripe, 230, 255]
    });
    let mut background = Background::new();
    background.set(&sky, 64, 32, WIDTH, HEIGHT);

    Assets {
        textures,
        meta,
        background,
    }
}

fn area(x: i32, y: i32, w: i32, h: i32, floor: bool, ceiling: bool, road: bool) -> Placement {
    Placement::Area(Area {
        x,
        y,
        w,
        h,
        floor,
        ceiling,
        road,
    })
}

// A segment along x (north) starting at (x, y), with the game's default thickness.
fn segment(kind: SegmentKind, x: f64, y: f64, length: f64) -> Placement {
    Placement::Segment(Segment {
        kind,
        direction: Direction::North,
        x,
        y,
        length,
        thickness: if kind == SegmentKind::Window {
            0.0
        } else {
            0.1
        },
    })
}

fn world(placements: &[Placement]) -> World {
    let mut world = World::new(12);
    for &placement in placements {
        world.add(placement).unwrap();
    }
    world
}

fn render_frame(world: &World, position: &Position) -> Vec<u8> {
    let assets = assets();
    let pixels = (WIDTH * HEIGHT) as usize;
    let mut image = vec![0; pixels * 4];
    let mut column_depth = vec![0.0; WIDTH as usize];
    let mut depth = vec![0.0; pixels];
    let mut pass_times = [0.0; RENDER_PASS_COUNT];
    render(
        position,
        world,
        &mut image,
        &mut column_depth,
        &mut depth,
        &mut pass_times,
        WIDTH,
        HEIGHT,
        LIGHT_RANGE,
        RANGE,
        0,
        &assets.background,
        &assets.textures,
        &assets.meta,
        || 0.0,
    );
    image
}

fn snapshot_path(name: &str, suffix: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/tests/snapshots")
        .join(format!("{name}{suffix}.png"))
}

fn write_png(path: &PathBuf, image: &[u8]) {
    let file = File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, WIDTH as u32, HEIGHT as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(image)
        .unwrap();
}

fn read_png(path: &PathBuf) -> Option<Vec<u8>> {
    let decoder = png::Decoder::new(File::open(path).ok()?);
    let mut reader = decoder.read_info().unwrap();
    let mut image = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut image).unwrap();
    assert_eq!((info.width, info.height), (WIDTH as u32, HEIGHT as u32));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    image.truncate(info.buffer_size());
    Some(image)
}

// Renders the scene twice, so threading cannot change the result, then compares it with
// the reference.
fn assert_snapshot(name: &str, world: &World, position: &Position) {
    let image = render_frame(world, position);
    assert!(
        image == render_frame(world, position),
        "{name}: two renders of the same scene differ"
    );

    let reference = snapshot_path(name, "");
    let actual = snapshot_path(name, ".actual");
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        write_png(&reference, &image);
        let _ = std::fs::remove_file(&actual);
        return;
    }
    let Some(expected) = read_png(&reference) else {
        panic!("{name}: no reference image; run UPDATE_SNAPSHOTS=1 cargo test snapshot");
    };
    if image != expected {
        write_png(&actual, &image);
        let different = image
            .chunks_exact(4)
            .zip(expected.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        panic!(
            "{name}: {different} of {} pixels differ from {}; the frame is in {}",
            WIDTH * HEIGHT,
            reference.display(),
            actual.display()
        );
    }
    let _ = std::fs::remove_file(&actual);
}

// Inside a room facing its north wall, which has a door and a window to a road outside.
#[test]
fn snapshot_walls_door_window() {
    let world = world(&[
        area(0, 0, 12, 3, true, false, true),
        area(0, 3, 12, 9, true, true, false),
        segment(SegmentKind::Wall, 1.0, 3.0, 2.0),
        segment(SegmentKind::Door, 3.0, 3.0, 1.0),
        segment(SegmentKind::Window, 4.0, 3.0, 2.0),
        segment(SegmentKind::Wall, 6.0, 3.0, 3.0),
    ]);
    assert_snapshot("walls_door_window", &world, &camera(4.5, 7.5, 0.0, 0));
}

// A tree sprite outside, seen through window glass.
#[test]
fn snapshot_sprite_behind_glass() {
    let mut world = world(&[
        area(0, 0, 12, 12, true, false, false),
        segment(SegmentKind::Window, 2.0, 5.0, 5.0),
    ]);
    world.add_sprite(SpritePlacement {
        x: 4.5,
        y: 3.0,
        texture: TextureType::TREE_CONE as i32,
        height: 1.0,
        angle: 0.0,
    });
    assert_snapshot("sprite_behind_glass", &world, &camera(4.5, 7.5, 0.0, 0));
}

// A unit cube in the file's own Y-up axes, with one normal per face.
fn cube() -> Mesh {
    let positions = vec![
        -0.5, 0.0, -0.5, 0.5, 0.0, -0.5, 0.5, 1.0, -0.5, -0.5, 1.0, -0.5, // back
        -0.5, 0.0, 0.5, 0.5, 0.0, 0.5, 0.5, 1.0, 0.5, -0.5, 1.0, 0.5, // front
    ];
    let faces: [([u32; 4], [f64; 3]); 6] = [
        ([4, 5, 6, 7], [0.0, 0.0, 1.0]),
        ([1, 0, 3, 2], [0.0, 0.0, -1.0]),
        ([5, 1, 2, 6], [1.0, 0.0, 0.0]),
        ([0, 4, 7, 3], [-1.0, 0.0, 0.0]),
        ([7, 6, 2, 3], [0.0, 1.0, 0.0]),
        ([0, 1, 5, 4], [0.0, -1.0, 0.0]),
    ];
    let mut triangles = Vec::new();
    let mut normals = Vec::new();
    for (quad, normal) in faces {
        for corner in [0, 1, 2, 0, 2, 3] {
            triangles.push(quad[corner]);
            normals.extend_from_slice(&normal);
        }
    }
    let colors = vec![f32::NAN; triangles.len()]; // r, g, b per triangle, all from the model
    Mesh {
        positions,
        triangles,
        normals,
        colors,
    }
}

// A turned cube model in front of a wall, under a ceiling.
#[test]
fn snapshot_mesh() {
    let mut world = world(&[
        area(0, 0, 12, 12, true, true, false),
        segment(SegmentKind::Wall, 1.0, 3.0, 7.0),
    ]);
    let mesh = world.add_mesh(cube());
    world
        .add(Placement::Model(Model {
            mesh,
            x: 4.5,
            y: 5.0,
            z: 0.0,
            scale: 1.0,
            height: Some(0.6),
            angle: 30.0,
            color: [0.8, 0.3, 0.2],
            solid: true,
        }))
        .unwrap();
    assert_snapshot("mesh", &world, &camera(4.5, 7.5, 0.0, 0));
}

// Flying above a roofed house and looking down at its roof, walls and the road around it.
#[test]
fn snapshot_roof_from_above() {
    let world = world(&[
        area(0, 0, 12, 12, true, false, true),
        area(2, 2, 5, 4, true, true, false),
        segment(SegmentKind::Wall, 2.0, 2.0, 5.0),
        segment(SegmentKind::Wall, 2.0, 5.9, 1.0),
        segment(SegmentKind::Window, 3.0, 5.9, 1.0),
        segment(SegmentKind::Wall, 4.0, 5.9, 3.0),
    ]);
    assert_snapshot("roof_from_above", &world, &camera(4.5, 9.5, 1.5, -40));
}
