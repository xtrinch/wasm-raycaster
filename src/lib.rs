// The wasm-bindgen API the TypeScript game calls. It turns JavaScript values and raw buffer
// pointers into engine types and slices, and engine results back into JavaScript values.
use engine::{Background, Mesh, Placement, Position, TextureMap, TextureMetaMap, World};
use js_sys::Float32Array;
use raycast_engine as engine;
use std::f32::consts::PI;
use std::slice::{from_raw_parts, from_raw_parts_mut};
use wasm_bindgen::prelude::*;

#[cfg(feature = "parallel")]
pub use wasm_bindgen_rayon::init_thread_pool;

// The engine's TextureType values, for TypeScript.
#[wasm_bindgen]
pub enum TextureType {
    WALL = 1,
    CEILING = 2,
    FLOOR = 3,
    ROAD = 4,
    DOOR = 5,
    TREE_CONE = 6,
    PILLAR = 7,
    BUSH1 = 8,
    TREE_VASE = 9,
    TREE_COLUMNAR = 10,
    LADY = 11,
    WINDOW = 12,
    ROOF = 13,
}

#[wasm_bindgen]
#[inline(never)]
pub fn render(
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    plane_x: f32,
    plane_y: f32,
    pitch: i32,
    z: f32,
    plane_y_initial: f32,
    render_img: *mut u8,
    column_depth_array: *mut f32,
    world: &WasmWorld, // grid, sprites and models
    width: i32,
    height: i32,
    light_range: i32,
    range: i8,
    map_light: i32,
    background: &BackgroundImageWasm,
    sprites_texture_map: &WasmTextureMap, // contains textures along with angled textures
    sprites_texture_meta_map: &WasmTextureMetaMap,
    depth_array: *mut f32, // Nearest floor, ceiling, roof or mesh depth per pixel
    pass_times_array: *mut f32, // RENDER_PASS_COUNT milliseconds, one per pass in run order
) {
    let position = Position {
        x,
        y,
        dir_x,
        dir_y,
        plane_x,
        plane_y,
        pitch,
        z,
        plane_y_initial,
        map_x: x.floor() as i32,
        map_y: y.floor() as i32,
    };
    let pixels = width as usize * height as usize;
    let img_slice = unsafe { from_raw_parts_mut(render_img, pixels * 4) };
    let column_depth = unsafe { from_raw_parts_mut(column_depth_array, width as usize) };
    let depth = unsafe { from_raw_parts_mut(depth_array, pixels) };
    let pass_times = unsafe { from_raw_parts_mut(pass_times_array, engine::RENDER_PASS_COUNT) };

    engine::render(
        &position,
        &world.0,
        img_slice,
        column_depth,
        depth,
        pass_times,
        width,
        height,
        light_range,
        range,
        map_light,
        &background.0,
        &sprites_texture_map.0,
        &sprites_texture_meta_map.0,
        now_ms,
    );
}

// Milliseconds from the page's high-resolution clock; render runs on the main thread.
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|window| window.performance())
        .map_or(0.0, |performance| performance.now())
}

// Apply vertical movement and collisions; return [feet height, landed flag].
#[wasm_bindgen]
pub fn move_vertical(
    x: f32,               // Player's world x coordinate.
    y: f32,               // Player's world y coordinate.
    z: f32,               // Feet height in world units.
    distance: f32,        // Requested vertical travel in world units; positive means up.
    fall_distance: f32,   // Downward travel to apply when gravity is active.
    jetpack_active: bool, // Hover instead of applying gravity while airborne.
    world: &WasmWorld,    // Grid and solid models.
) -> Float32Array {
    let (next_z, landed) =
        engine::move_vertical(x, y, z, distance, fall_distance, jetpack_active, &world.0);
    Float32Array::from(&[next_z, if landed { 1.0 } else { 0.0 }][..])
}

// Move along the view direction unless a wall, window or solid model is in the way.
#[wasm_bindgen]
pub fn walk(
    x: f32,
    y: f32,
    dir_x: f32,
    dir_y: f32,
    z: f32,            // Feet height; feet at or above a box's top pass over it.
    distance: f32,     // Requested travel in world units; negative walks back.
    world: &WasmWorld, // Grid and solid models.
) -> Float32Array {
    let (x, y) = engine::walk(x, y, dir_x, dir_y, z, distance, &world.0);
    Float32Array::from(&[x, y][..])
}

#[wasm_bindgen]
pub fn rotate_view(
    frame_time: f32,
    multiplier: i32,
    dir_x: f32,
    dir_y: f32,
    plane_x: f32,
    plane_y: f32,
) -> Float32Array {
    let rot_speed = 0.8 * PI * frame_time * multiplier as f32;

    let (sin_r, cos_r) = rot_speed.sin_cos(); // more efficient than separate .sin() and .cos()

    let new_dir_x = dir_x * cos_r - dir_y * sin_r;
    let new_dir_y = dir_x * sin_r + dir_y * cos_r;

    let new_plane_x = plane_x * cos_r - plane_y * sin_r;
    let new_plane_y = plane_x * sin_r + plane_y * cos_r;

    // Avoid heap allocation by creating a fixed-size array
    Float32Array::from(&[new_dir_x, new_dir_y, new_plane_x, new_plane_y][..])
}

#[wasm_bindgen]
pub struct WasmTextureMetaMap(TextureMetaMap);

#[wasm_bindgen]
impl WasmTextureMetaMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(TextureMetaMap::new())
    }

    #[wasm_bindgen(js_name = populateFromArray)]
    pub fn populate_from_array(&mut self, key: i32, width: i32, height: i32, angles: u32) {
        self.0.insert(key, width, height, angles);
    }
}

#[wasm_bindgen]
pub struct WasmTextureMap(TextureMap);

#[wasm_bindgen]
impl WasmTextureMap {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(TextureMap::new())
    }

    #[wasm_bindgen(js_name = populateFromArray)]
    pub fn populate_from_array(&mut self, key0: i32, angle: i32, sprite_data: &[u8]) {
        self.0.insert(key0, angle, sprite_data);
    }

    #[wasm_bindgen]
    pub fn count_cells(&self) -> usize {
        self.0.len()
    }
}

#[wasm_bindgen]
pub struct BackgroundImageWasm(Background);

#[wasm_bindgen]
impl BackgroundImageWasm {
    #[wasm_bindgen]
    pub fn set(
        &mut self,
        bg_img_texture: *const u8,
        texture_width: i32,
        texture_height: i32,
        screen_width: i32,
        screen_height: i32,
    ) {
        let src = unsafe {
            from_raw_parts(
                bg_img_texture,
                (texture_width * texture_height * 4) as usize,
            )
        };
        self.0.set(
            src,
            texture_width,
            texture_height,
            screen_width,
            screen_height,
        );
    }

    #[wasm_bindgen(constructor)]
    pub fn new() -> BackgroundImageWasm {
        BackgroundImageWasm(Background::new())
    }
}

// The level as JavaScript sees it: placements go in, ids come back for removing and moving
// them while the game runs. Rendering and movement read the world directly.
#[wasm_bindgen]
pub struct WasmWorld(World);

#[wasm_bindgen]
impl WasmWorld {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        Self(World::new(size))
    }

    /// An area, segment, sprite or model placement as built by levelBuilder.ts. Returns its
    /// id, or 0 for areas, which cannot be removed. Throws when the grid cannot hold it.
    pub fn add(&mut self, placement: JsValue) -> Result<u32, JsError> {
        let placement: Placement = serde_wasm_bindgen::from_value(placement)
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.0.add(placement).map_err(|error| JsError::new(&error))
    }

    /// A model's geometry in its file's own axes, uploaded once and placed by the returned
    /// id: x, y, z per vertex; three vertex indices per triangle; x, y, z normal per
    /// triangle corner; r, g, b per triangle, NaN where it has no material colour.
    #[wasm_bindgen(js_name = addMesh)]
    pub fn add_mesh(
        &mut self,
        positions: &[f64],
        triangles: &[u32],
        normals: &[f64],
        colors: &[f32],
    ) -> u32 {
        self.0.add_mesh(Mesh {
            positions: positions.to_vec(),
            triangles: triangles.to_vec(),
            normals: normals.to_vec(),
            colors: colors.to_vec(),
        })
    }

    /// Removes any placement but an area; false if the id is unknown.
    pub fn remove(&mut self, id: u32) -> bool {
        self.0.remove(id)
    }

    /// Moves a sprite or model to world x/y; false for walls, doors, windows or unknown ids.
    #[wasm_bindgen(js_name = move)]
    pub fn move_to(&mut self, id: u32, x: f64, y: f64) -> bool {
        self.0.move_to(id, x, y)
    }

    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.0.size()
    }

    /// A copy of the packed grid, row-major; see gridMap.ts for the bit layout.
    pub fn grid(&self) -> js_sys::BigUint64Array {
        js_sys::BigUint64Array::from(self.0.grid())
    }
}

#[wasm_bindgen]
pub struct WasmUint8Array(Vec<u8>);

#[wasm_bindgen]
impl WasmUint8Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Uint8Array {
        unsafe { js_sys::Uint8Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }

    // set data from a JavaScript Uint8Array
    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Uint8Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmFloat32Array {
    buffer: Vec<f32>,
}

#[wasm_bindgen]
impl WasmFloat32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
        }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> Float32Array {
        unsafe { Float32Array::view_mut_raw(self.buffer.as_mut_ptr(), self.buffer.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut f32 {
        self.buffer.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: Float32Array) {
        let len = self.buffer.len().min(data.length() as usize);
        self.buffer[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmInt32Array(Vec<i32>);

#[wasm_bindgen]
impl WasmInt32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Int32Array {
        unsafe { js_sys::Int32Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut i32 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Int32Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmUInt32Array(Vec<u32>);

#[wasm_bindgen]
impl WasmUInt32Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::Uint32Array {
        unsafe { js_sys::Uint32Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u32 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::Uint32Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[wasm_bindgen]
pub struct WasmUInt64Array(Vec<u64>);

#[wasm_bindgen]
impl WasmUInt64Array {
    #[wasm_bindgen(constructor)]
    pub fn new(size: usize) -> Self {
        let buffer = vec![0; size];
        Self { 0: buffer }
    }

    #[wasm_bindgen(getter, js_name = buffer)]
    pub fn buffer(&mut self) -> js_sys::BigUint64Array {
        unsafe { js_sys::BigUint64Array::view_mut_raw(self.0.as_mut_ptr(), self.0.len()) }
    }

    #[wasm_bindgen(getter, js_name = ptr)]
    pub fn ptr(&mut self) -> *mut u64 {
        self.0.as_mut_ptr()
    }

    #[wasm_bindgen]
    pub fn set(&mut self, data: js_sys::BigUint64Array) {
        let len = self.0.len().min(data.length() as usize);
        self.0[..len].copy_from_slice(&data.to_vec()[..len]);
    }
}

#[cfg(test)]
#[path = "tests/texture_type.rs"]
mod texture_type_tests;
