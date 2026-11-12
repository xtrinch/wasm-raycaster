// The level, kept in Rust: areas, walls, doors, windows, pillars, sprites and models, each
// added with an id so it can be removed or moved while the game runs. Every edit re-encodes
// only the cells it touches into the packed grid that rendering and movement read; the bit
// layout is documented in gridMap.ts. Positions are world units (one cell is 1 by 1);
// wall, door and window geometry must use multiples of 0.1, the grid's precision.

use crate::helpers::{footprint_cells, Cell, CollisionBox, CollisionCells, Sprite};
use crate::world_helpers::*;
#[cfg(test)]
use serde::Deserialize;
use std::collections::HashMap;

pub struct World {
    size: usize,
    grid: Vec<u64>, // packed cells, row-major
    cells: Vec<CellState>,
    items: HashMap<u32, Item>,
    next_id: u32,
    meshes: Vec<Mesh>,
    sprites: HashMap<Cell, CellSprites>, // sprites standing in each cell
    models: HashMap<Cell, Vec<u32>>,     // ids of the models whose footprint touches each cell
    solids: CollisionCells,              // boxes of solid models, by model id
}

impl World {
    pub fn new(size: usize) -> Self {
        Self {
            size,
            grid: vec![0; size * size],
            cells: vec![CellState::default(); size * size],
            items: HashMap::new(),
            next_id: 1,
            meshes: Vec::new(),
            sprites: HashMap::new(),
            models: HashMap::new(),
            solids: CollisionCells::new(),
        }
    }

    // Returns the new item's id, or 0 for areas, which only set flags and cannot be removed.
    pub fn add(&mut self, placement: Placement) -> Result<u32, String> {
        match placement {
            Placement::Area(area) => self.set_area(area).map(|_| 0),
            Placement::Segment(segment) => self.add_segment(segment),
            Placement::Sprite(placement) => Ok(self.add_sprite(placement)),
            Placement::Model(model) => self.add_model(model),
        }
    }

    pub fn set_area(&mut self, area: Area) -> Result<(), String> {
        let mut indexes = Vec::new();
        for y in area.y..area.y + area.h {
            for x in area.x..area.x + area.w {
                indexes.push(self.index(x as i64, y as i64, "area")?);
            }
        }
        let flags = Flags {
            floor: area.floor,
            ceiling: area.ceiling,
            road: area.road,
        };
        for index in indexes {
            self.cells[index].flags = flags;
            self.encode(index);
        }
        Ok(())
    }

    // Splits the segment into the cells it crosses. Nothing changes unless every cell it
    // touches can still hold its segments.
    pub fn add_segment(&mut self, segment: Segment) -> Result<u32, String> {
        let what = format!(
            "{} at ({}, {})",
            match segment.kind {
                SegmentKind::Wall => "wall",
                SegmentKind::Door => "door",
                SegmentKind::Window => "window",
            },
            segment.x,
            segment.y
        );
        let east = segment.direction == Direction::East;
        let position = format!("{what} position");
        let across = tenths(if east { segment.x } else { segment.y }, &position)?;
        let along = tenths(if east { segment.y } else { segment.x }, &position)?;
        let length = tenths(segment.length, &format!("{what} length"))?;
        let thickness = tenths(segment.thickness, &format!("{what} thickness"))?;
        if length <= 0 {
            return Err(format!("{what} needs a positive length"));
        }

        // A flat segment on a cell line goes into the cell after the line.
        let across_pieces = if thickness > 0 {
            split_range(across, thickness)
        } else {
            let cell = across.div_euclid(10);
            vec![(cell, across - cell * 10, 0)]
        };

        let id = self.next_id;
        let mut placed: Vec<(usize, Piece)> = Vec::new();
        for &(across_cell, offset, size) in &across_pieces {
            for (along_cell, start, piece_length) in split_range(along, length) {
                let (x, y) = if east {
                    (across_cell, along_cell)
                } else {
                    (along_cell, across_cell)
                };
                let index = self.index(x, y, &what)?;
                placed.push((
                    index,
                    Piece {
                        id,
                        kind: segment.kind,
                        direction: segment.direction,
                        offset: offset as u64,
                        thickness: size as u64,
                        length: piece_length as u64,
                        start: start as u64,
                    },
                ));
            }
        }

        // Check every touched cell before changing any of them.
        let mut updates = Vec::with_capacity(placed.len());
        for &(index, piece) in &placed {
            let mut pieces = self.cells[index].pieces.clone();
            pieces.push(piece);
            let where_ = format!("cell ({}, {})", index % self.size, index / self.size);
            if pieces.len() > 3 {
                return Err(format!(
                    "{where_} needs {} segments; a cell holds at most 3",
                    pieces.len()
                ));
            }
            let value = encode_any_order(self.cells[index].flags, &pieces)
                .map_err(|error| format!("{where_}: {error}"))?;
            updates.push((index, pieces, value));
        }

        for (index, pieces, value) in updates {
            self.cells[index].pieces = pieces;
            self.grid[index] = value;
        }
        self.next_id += 1;
        self.items.insert(
            id,
            Item::Segment {
                cells: placed.iter().map(|&(index, _)| index).collect(),
            },
        );
        Ok(id)
    }

    // Sprites may stand outside the grid.
    pub fn add_sprite(&mut self, placement: SpritePlacement) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        let sprite = Sprite {
            x: placement.x as f32,
            y: placement.y as f32,
            fract: 0.,
            dx: 0.,
            dy: 0.,
            angle: placement.angle as f32 as i32,
            // Sprite heights are percentages of one wall height.
            height: (placement.height * 100.0) as f32 as i32,
            r#type: placement.texture,
            column: 0,
            distance: 0.,
            distance_fixed: 0,
        };
        let cell = self.place_sprite(id, sprite);
        self.items.insert(id, Item::Sprite { cell });
        id
    }

    pub fn add_mesh(&mut self, mesh: Mesh) -> u32 {
        self.meshes.push(mesh);
        (self.meshes.len() - 1) as u32
    }

    pub fn add_model(&mut self, model: Model) -> Result<u32, String> {
        let mesh = self
            .meshes
            .get(model.mesh as usize)
            .ok_or_else(|| format!("model mesh {} was never added", model.mesh))?;
        let placed = bake_model(mesh, model);
        let id = self.next_id;
        self.next_id += 1;
        self.register_model(id, &placed);
        self.items.insert(id, Item::Model(placed));
        Ok(id)
    }

    // Removes any placement but an area; false if the id is unknown.
    pub fn remove(&mut self, id: u32) -> bool {
        let Some(item) = self.items.remove(&id) else {
            return false;
        };
        match item {
            Item::Segment { cells } => {
                for index in cells {
                    self.cells[index].pieces.retain(|piece| piece.id != id);
                    self.encode(index);
                }
            }
            Item::Sprite { cell } => self.unplace_sprite(id, cell),
            Item::Model(placed) => self.unregister_model(id, &placed),
        }
        true
    }

    // Moves a sprite or model to world x/y; false for walls, doors, windows or unknown ids.
    pub fn move_to(&mut self, id: u32, x: f64, y: f64) -> bool {
        match self.items.get(&id) {
            Some(&Item::Sprite { cell }) => {
                let index = self.sprites[&cell].ids.iter().position(|&i| i == id);
                let mut sprite = self.sprites[&cell].sprites[index.unwrap()].clone();
                self.unplace_sprite(id, cell);
                sprite.x = x as f32;
                sprite.y = y as f32;
                let cell = self.place_sprite(id, sprite);
                self.items.insert(id, Item::Sprite { cell });
                true
            }
            Some(Item::Model(placed)) => {
                let model = Model {
                    x,
                    y,
                    ..placed.model
                };
                let Some(Item::Model(old)) = self.items.remove(&id) else {
                    unreachable!()
                };
                self.unregister_model(id, &old);
                let placed = bake_model(&self.meshes[model.mesh as usize], model);
                self.register_model(id, &placed);
                self.items.insert(id, Item::Model(placed));
                true
            }
            _ => false,
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn grid(&self) -> &[u64] {
        &self.grid
    }

    pub fn sprites_at(&self, cell: &Cell) -> &[Sprite] {
        self.sprites
            .get(cell)
            .map_or(&[], |in_cell| in_cell.sprites.as_slice())
    }

    pub fn models_at(&self, cell: &Cell) -> &[u32] {
        self.models
            .get(cell)
            .map_or(&[], |models| models.as_slice())
    }

    // Whether a sprite or a model occupies the cell, so rays should record it.
    pub fn occupied(&self, cell: &Cell) -> bool {
        self.sprites.contains_key(cell) || self.models.contains_key(cell)
    }

    // A model's world-space triangles, MESH_TRIANGLE_STRIDE floats each.
    pub fn model_triangles(&self, id: u32) -> Option<&[f32]> {
        match self.items.get(&id) {
            Some(Item::Model(placed)) => Some(&placed.triangles),
            _ => None,
        }
    }

    pub fn solids(&self) -> &CollisionCells {
        &self.solids
    }

    fn index(&self, x: i64, y: i64, what: &str) -> Result<usize, String> {
        let size = self.size as i64;
        if x < 0 || y < 0 || x >= size || y >= size {
            return Err(format!(
                "{what} reaches cell ({x}, {y}) outside the {size}x{size} grid"
            ));
        }
        Ok((y * size + x) as usize)
    }

    // Re-packs a cell whose flags changed or which lost a segment. Any one or two segments
    // fit a cell, so a cell that held its segments before always holds them after.
    fn encode(&mut self, index: usize) {
        let cell = &self.cells[index];
        self.grid[index] = encode_any_order(cell.flags, &cell.pieces)
            .expect("a cell's remaining segments always fit");
    }

    fn place_sprite(&mut self, id: u32, sprite: Sprite) -> Cell {
        let cell = (sprite.x.floor() as i32, sprite.y.floor() as i32);
        let in_cell = self.sprites.entry(cell).or_default();
        in_cell.sprites.push(sprite);
        in_cell.ids.push(id);
        cell
    }

    fn unplace_sprite(&mut self, id: u32, cell: Cell) {
        let in_cell = self.sprites.get_mut(&cell).unwrap();
        let index = in_cell.ids.iter().position(|&i| i == id).unwrap();
        in_cell.sprites.remove(index);
        in_cell.ids.remove(index);
        if in_cell.ids.is_empty() {
            self.sprites.remove(&cell);
        }
    }

    fn register_model(&mut self, id: u32, placed: &PlacedModel) {
        let [min_x, min_y, max_x, max_y] = placed.footprint;
        for cell in footprint_cells(min_x, min_y, max_x, max_y) {
            self.models.entry(cell).or_default().push(id);
        }
        if placed.model.solid {
            self.solids.insert(
                id,
                CollisionBox {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                    top: placed.top,
                },
            );
        }
    }

    fn unregister_model(&mut self, id: u32, placed: &PlacedModel) {
        let [min_x, min_y, max_x, max_y] = placed.footprint;
        for cell in footprint_cells(min_x, min_y, max_x, max_y) {
            if let Some(ids) = self.models.get_mut(&cell) {
                ids.retain(|&other| other != id);
                if ids.is_empty() {
                    self.models.remove(&cell);
                }
            }
        }
        self.solids.remove(id);
    }
}

#[cfg(test)]
#[path = "tests/world.rs"]
mod world_tests;
