use super::*;
use crate::helpers::SOLID_STRIDE;

// Cell values from the examples in gridMap.ts: full-length segments at offset 0.
const NORTH_WALL: u64 = 0x000000000A101047; // along x, thickness 0.1
const EAST_DOOR: u64 = 0x000000000A101027; // along y, thickness 0.1
const EAST_WINDOW: u64 = 0x000000000A001207; // along y, thickness 0
const INDOORS: u64 = 0x6;

// A bed-sized solid model: footprint x 1..1.4, y 1..2, top at 0.25.
const BED: [f32; SOLID_STRIDE] = [1.0, 1.0, 1.4, 2.0, 0.25];

// A 3 by 3 grid of indoor cells with one segment in the middle cell (1, 1).
fn grid(middle: u64) -> Vec<u64> {
    let mut cells = vec![INDOORS; 9];
    cells[4] = middle;
    cells
}

fn solids(data: &[f32]) -> WasmCollisionCells {
    let mut collision = WasmCollisionCells::new();
    collision.populate_solids(data);
    collision
}

fn step(
    from: (f32, f32),
    delta: (f32, f32),
    z: f32,
    cells: &[u64],
    collision: &WasmCollisionCells,
) -> (f32, f32) {
    walk_step(from.0, from.1, delta.0, delta.1, z, cells, 3, collision)
}

#[test]
fn wall_slots_become_world_boxes() {
    let boxes: Vec<CollisionBox> = wall_boxes(NORTH_WALL, 1, 1).collect();
    assert_eq!(boxes.len(), 1);
    let b = boxes[0];
    assert_eq!((b.min_x, b.max_x, b.top), (1.0, 2.0, 1.0));
    assert!((b.min_y - 1.0).abs() < 1e-6 && (b.max_y - 1.1).abs() < 1e-6);
    // A full-cell block is one box covering the cell.
    let full: Vec<CollisionBox> = wall_boxes(1, 2, 0).collect();
    assert_eq!(
        (full[0].min_x, full[0].min_y, full[0].max_x, full[0].max_y),
        (2.0, 0.0, 3.0, 1.0)
    );
}

#[test]
fn doors_do_not_collide_but_still_support_roof_walking() {
    assert_eq!(wall_boxes(EAST_DOOR, 1, 1).count(), 0);
    assert!(inside_wall_footprint(EAST_DOOR, 0.05, 0.5));
    let none = solids(&[]);
    assert_eq!(
        step((0.5, 1.5), (1.0, 0.0), 0.0, &grid(EAST_DOOR), &none),
        (1.5, 1.5)
    );
}

#[test]
fn walls_and_windows_stop_the_player_at_its_radius() {
    let none = solids(&[]);
    // Walking south into the north wall at y 1..1.1 stops 0.2 short of it.
    assert_eq!(
        step((1.5, 0.75), (0.0, 0.1), 0.0, &grid(NORTH_WALL), &none),
        (1.5, 0.75)
    );
    assert_eq!(
        step((1.5, 0.65), (0.0, 0.1), 0.0, &grid(NORTH_WALL), &none),
        (1.5, 0.75)
    );
    // A zero-thickness window at x 1 blocks like a wall.
    assert_eq!(
        step((0.5, 1.5), (1.0, 0.0), 0.0, &grid(EAST_WINDOW), &none),
        (0.5, 1.5)
    );
}

#[test]
fn diagonal_steps_slide_along_walls() {
    let none = solids(&[]);
    assert_eq!(
        step((1.5, 0.75), (0.1, 0.1), 0.0, &grid(NORTH_WALL), &none),
        (1.6, 0.75)
    );
}

#[test]
fn feet_at_or_above_a_top_pass_over_it() {
    let none = solids(&[]);
    assert_eq!(
        step((1.5, 0.5), (0.0, 1.0), 1.0, &grid(NORTH_WALL), &none),
        (1.5, 1.5)
    );
    let bed = solids(&BED);
    assert_eq!(
        step((1.7, 1.5), (-0.5, 0.0), 0.25, &grid(INDOORS), &bed),
        (1.2, 1.5)
    );
    assert_eq!(
        step((1.7, 1.5), (-0.5, 0.0), 0.2, &grid(INDOORS), &bed),
        (1.7, 1.5)
    );
}

#[test]
fn long_steps_cannot_pass_through_a_solid_model() {
    let bed = solids(&BED);
    // A slow frame's step from one side of the bed to the other.
    assert_eq!(
        step((0.5, 1.5), (1.5, 0.0), 0.0, &grid(INDOORS), &bed),
        (0.5, 1.5)
    );
}

#[test]
fn a_player_inside_a_solid_model_can_walk_out() {
    let bed = solids(&BED);
    let (x, y) = step((1.2, 1.5), (0.6, 0.0), 0.0, &grid(INDOORS), &bed);
    assert!((x - 1.8).abs() < 1e-6 && y == 1.5);
}

#[test]
fn solid_models_are_found_in_every_cell_they_touch() {
    let bed = solids(&[0.8, 0.5, 1.4, 2.0, 0.25]);
    for cell in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        assert_eq!(bed.boxes_in(cell, cell).len(), 1, "cell {cell:?}");
    }
    // The footprint ends on the line y = 2, so it does not reach row 2.
    assert!(bed.boxes_in((1, 2), (1, 2)).is_empty());
}

#[test]
fn a_solid_spanning_several_cells_is_returned_once() {
    let collision = solids(&[0.8, 0.5, 1.4, 2.0, 0.25, 0.2, 0.2, 0.4, 0.4, 0.5]);
    let boxes = collision.boxes_in((0, 0), (1, 1));
    assert_eq!(boxes.len(), 2);
    assert_eq!((boxes[0].top, boxes[1].top), (0.25, 0.5));
}

#[test]
fn solid_tops_support_feet_above_them() {
    let bed = solids(&BED);
    let here = bed.boxes_in((1, 1), (1, 1));
    assert_eq!(solid_support_at(1.2, 1.5, 0.3, &here), 0.25);
    assert_eq!(solid_support_at(1.2, 1.5, 0.25, &here), 0.25);
    // Below the top or beside the footprint, feet stand on the ground.
    assert_eq!(solid_support_at(1.2, 1.5, 0.1, &here), 0.0);
    assert_eq!(solid_support_at(1.6, 1.5, 0.3, &here), 0.0);
}
