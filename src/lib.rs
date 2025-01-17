use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn wasm_test() -> String {
    "Hello, wasm!".to_string()
}
