use super::*;

// level.ts's areas, walls, doors, windows, pillars and sprites with the grid and sprite list
// the TypeScript compileLevel produced from them before levels moved into World.
#[derive(Deserialize)]
struct LevelFixture {
    size: usize,
    placements: Vec<Placement>,
    grid: Vec<String>,      // hex cell values, row-major
    sprites: Vec<[f64; 5]>, // x, y, angle, height, texture
}

fn fixture() -> LevelFixture {
    serde_json::from_str(include_str!("fixtures/level.json")).unwrap()
}

fn load(fixture: &LevelFixture) -> World {
    let mut world = World::new(fixture.size);
    for placement in &fixture.placements {
        world.add(*placement).unwrap();
    }
    world
}

fn segment(kind: SegmentKind, x: f64, y: f64, direction: Direction, length: f64) -> Segment {
    Segment {
        kind,
        direction,
        x,
        y,
        length,
        thickness: if kind == SegmentKind::Window {
            0.0
        } else {
            0.1
        },
    }
}

fn wall(x: f64, y: f64, direction: Direction, length: f64) -> Segment {
    segment(SegmentKind::Wall, x, y, direction, length)
}

fn window(x: f64, y: f64, direction: Direction, length: f64) -> Segment {
    segment(SegmentKind::Window, x, y, direction, length)
}

fn tree(x: f64, y: f64) -> SpritePlacement {
    SpritePlacement {
        x,
        y,
        texture: 6,
        height: 1.5,
        angle: 0.0,
    }
}

// Only bounds, no triangles: file x from -half_x to half_x, height (file y) 0 to height,
// file z from -half_z to half_z; unrotated, those become world x, z and y.
fn bounds_mesh(half_x: f64, height: f64, half_z: f64) -> Mesh {
    Mesh {
        positions: vec![-half_x, 0.0, -half_z, half_x, height, half_z],
        triangles: vec![],
        normals: vec![],
        colors: vec![],
    }
}

fn model(mesh: u32, x: f64, y: f64, solid: bool) -> Model {
    Model {
        mesh,
        x,
        y,
        z: 0.0,
        scale: 1.0,
        height: None,
        angle: 0.0,
        color: [0.8; 3],
        solid,
    }
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_level_packs_the_same_grid_as_the_typescript_builder() {
    let fixture = fixture();
    let world = load(&fixture);
    let expected: Vec<u64> = fixture
        .grid
        .iter()
        .map(|value| u64::from_str_radix(&value[2..], 16).unwrap())
        .collect();
    assert_eq!(world.grid().len(), expected.len());
    for (index, (&got, &want)) in world.grid().iter().zip(&expected).enumerate() {
        let (x, y) = (index % fixture.size, index / fixture.size);
        assert_eq!(got, want, "cell ({x}, {y}): {got:#x} instead of {want:#x}");
    }
}

#[test]
fn the_level_places_the_same_sprites_as_the_typescript_builder() {
    let fixture = fixture();
    let world = load(&fixture);
    // As camera.ts passed them on: heights in percent, through a Float32Array.
    let mut expected: HashMap<Cell, Vec<(f32, f32, i32, i32, i32)>> = HashMap::new();
    for &[x, y, angle, height, texture] in &fixture.sprites {
        let (x, y) = (x as f32, y as f32);
        expected
            .entry((x.floor() as i32, y.floor() as i32))
            .or_default()
            .push((
                x,
                y,
                angle as f32 as i32,
                (height * 100.0) as f32 as i32,
                texture as i32,
            ));
    }
    for (cell, sprites) in &expected {
        let placed: Vec<_> = world
            .sprites_at(cell)
            .iter()
            .map(|s| (s.x, s.y, s.angle, s.height, s.r#type))
            .collect();
        assert_eq!(&placed, sprites, "cell {cell:?}");
    }
    let total: usize = world.sprites.values().map(|b| b.sprites.len()).sum();
    assert_eq!(total, fixture.sprites.len());
}

#[test]
fn a_rejected_segment_changes_nothing() {
    let mut world = World::new(3);
    for y in [1.0, 1.3, 1.6] {
        world
            .add_segment(wall(2.0, y, Direction::North, 1.0))
            .unwrap();
    }
    let before = world.grid().to_vec();
    // Its first piece fits cell (1, 1), but cell (2, 1) is full.
    assert_eq!(
        world.add_segment(wall(1.0, 1.8, Direction::North, 2.0)),
        Err("cell (2, 1) needs 4 segments; a cell holds at most 3".to_string())
    );
    assert_eq!(world.grid(), before);
}

#[test]
fn invalid_segments_report_the_typescript_errors() {
    let mut world = World::new(3);
    assert_eq!(
        world.add_segment(wall(1.05, 1.0, Direction::North, 1.0)),
        Err("wall at (1.05, 1) position 1.05 is not a multiple of 0.1".to_string())
    );
    assert_eq!(
        world.add_segment(wall(2.0, 1.0, Direction::North, 2.0)),
        Err("wall at (2, 1) reaches cell (3, 1) outside the 3x3 grid".to_string())
    );
    assert_eq!(
        world.add_segment(wall(1.0, 1.0, Direction::North, 0.0)),
        Err("wall at (1, 1) needs a positive length".to_string())
    );
    for y in [1.0, 1.3] {
        world
            .add_segment(window(1.0, y, Direction::North, 1.0))
            .unwrap();
    }
    assert_eq!(
        world.add_segment(window(1.0, 1.6, Direction::North, 1.0)),
        Err("cell (1, 1): Only segments 0 and 1 can be windows".to_string())
    );
    assert_eq!(
        world.add(Placement::Area(Area {
            x: 2,
            y: 2,
            w: 2,
            h: 1,
            floor: true,
            ceiling: false,
            road: false,
        })),
        Err("area reaches cell (3, 2) outside the 3x3 grid".to_string())
    );
    // The failed area set no flags.
    assert_eq!(world.grid()[2 * 3 + 2], 0);
}

#[test]
fn windows_move_to_a_slot_that_can_hold_them() {
    let mut world = World::new(3);
    world
        .add_segment(wall(1.0, 1.0, Direction::North, 1.0))
        .unwrap();
    world
        .add_segment(wall(1.0, 1.3, Direction::North, 1.0))
        .unwrap();
    world
        .add_segment(window(1.0, 1.6, Direction::North, 1.0))
        .unwrap();
    let cell = world.grid()[4];
    // Slot 2 cannot be a window, so the window takes slot 1.
    assert_eq!(get_bits_of(cell, 12), 3);
    assert!(cell & (1 << 10) != 0 && cell & (1 << 9) == 0);
}

fn get_bits_of(value: u64, start: u64) -> u64 {
    (value >> start) & 0xf
}

#[test]
fn removing_a_segment_restores_its_cells() {
    let mut world = World::new(3);
    world
        .set_area(Area {
            x: 0,
            y: 0,
            w: 3,
            h: 3,
            floor: true,
            ceiling: true,
            road: false,
        })
        .unwrap();
    let before = world.grid().to_vec();
    let id = world
        .add_segment(wall(0.0, 1.0, Direction::North, 3.0))
        .unwrap();
    assert_ne!(world.grid(), before);
    assert!(world.remove(id));
    assert_eq!(world.grid(), before);
    // A stale id removes nothing.
    assert!(!world.remove(id));
}

#[test]
fn removing_a_segment_keeps_the_others_in_its_cells() {
    let mut world = World::new(3);
    let first = world
        .add_segment(wall(1.0, 1.0, Direction::North, 1.0))
        .unwrap();
    world
        .add_segment(window(1.0, 1.5, Direction::North, 1.0))
        .unwrap();
    world.remove(first);

    let mut expected = World::new(3);
    expected
        .add_segment(window(1.0, 1.5, Direction::North, 1.0))
        .unwrap();
    assert_eq!(world.grid(), expected.grid());
}

#[test]
fn sprites_move_between_cells_and_can_be_removed() {
    let mut world = World::new(3);
    let id = world.add_sprite(tree(0.5, 0.5));
    assert_eq!(world.sprites_at(&(0, 0))[0].height, 150);

    assert!(world.move_to(id, 2.5, 1.5));
    assert!(world.sprites_at(&(0, 0)).is_empty() && !world.occupied(&(0, 0)));
    assert_eq!(world.sprites_at(&(2, 1))[0].x, 2.5);

    assert!(world.remove(id));
    assert!(!world.occupied(&(2, 1)));
    assert!(!world.move_to(id, 1.0, 1.0));

    // Sprites may stand outside the grid, like the trees around the level.
    world.add_sprite(tree(-1.0, 0.5));
    assert_eq!(world.sprites_at(&(-1, 0)).len(), 1);
}

#[test]
fn walls_cannot_be_moved() {
    let mut world = World::new(3);
    let id = world
        .add_segment(wall(1.0, 1.0, Direction::North, 1.0))
        .unwrap();
    let before = world.grid().to_vec();
    assert!(!world.move_to(id, 2.0, 1.0));
    assert_eq!(world.grid(), before);
}

#[test]
fn solid_models_collide_where_they_stand() {
    let mut world = World::new(4);
    let mesh = world.add_mesh(bounds_mesh(0.2, 0.25, 0.5));
    // Footprint x 1..1.4, y 1..2, top 0.25.
    let bed = world.add_model(model(mesh, 1.2, 1.5, true)).unwrap();
    let boxes = world.solids().boxes_in((1, 1), (1, 1));
    assert_eq!(boxes.len(), 1);
    let b = boxes[0];
    assert!(close(b.min_x, 1.0) && close(b.max_x, 1.4) && close(b.min_y, 1.0));
    assert!(close(b.max_y, 2.0) && close(b.top, 0.25));

    assert!(world.move_to(bed, 2.2, 1.5));
    assert!(world.solids().boxes_in((1, 1), (1, 1)).is_empty());
    assert!(world.models_at(&(1, 1)).is_empty());
    assert_eq!(world.solids().boxes_in((2, 1), (2, 1)).len(), 1);
    assert_eq!(world.models_at(&(2, 1)), &[bed]);

    assert!(world.remove(bed));
    assert!(world.solids().boxes_in((0, 0), (3, 3)).is_empty());
    assert!(world.model_triangles(bed).is_none() && !world.occupied(&(2, 1)));
}

#[test]
fn models_that_are_not_solid_do_not_collide() {
    let mut world = World::new(4);
    let mesh = world.add_mesh(bounds_mesh(0.2, 0.25, 0.5));
    let id = world.add_model(model(mesh, 1.2, 1.5, false)).unwrap();
    assert!(world.solids().boxes_in((0, 0), (3, 3)).is_empty());
    assert_eq!(world.models_at(&(1, 1)), &[id]);
}

#[test]
fn a_model_needs_an_added_mesh() {
    let mut world = World::new(4);
    assert_eq!(
        world.add_model(model(0, 1.0, 1.0, false)),
        Err("model mesh 0 was never added".to_string())
    );
}

#[test]
fn models_stand_on_z_centred_on_x_y_and_scaled_to_their_height() {
    let mut world = World::new(8);
    // One triangle, 1 wide along file x and 2 tall along file y, facing file +z.
    let mesh = world.add_mesh(Mesh {
        positions: vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0],
        triangles: vec![0, 1, 2],
        normals: vec![0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
        colors: vec![f32::NAN; 3],
    });
    let id = world
        .add_model(Model {
            height: Some(1.0),
            ..model(mesh, 3.0, 4.0, true)
        })
        .unwrap();

    // Half scale; file +z becomes world +y.
    let corner = |x: f32, y: f32, z: f32| [x, y, z, 0.0, 1.0, 0.0];
    let expected: Vec<f32> = [
        corner(2.75, 4.0, 0.0),
        corner(3.25, 4.0, 0.0),
        corner(2.75, 4.0, 1.0),
    ]
    .concat()
    .into_iter()
    .chain([0.8; 3])
    .collect();
    let triangles = world.model_triangles(id).unwrap();
    assert_eq!(triangles.len(), expected.len());
    for (got, want) in triangles.iter().zip(&expected) {
        assert!(close(*got, *want), "{triangles:?}");
    }
    let b = world.solids().boxes_in((2, 4), (3, 4))[0];
    assert!(close(b.min_x, 2.75) && close(b.max_x, 3.25) && close(b.top, 1.0));
}
