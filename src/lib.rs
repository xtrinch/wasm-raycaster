use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn wasm_test() -> String {
    "Hello Mojca, from wasm!".to_string()
}
