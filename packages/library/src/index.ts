// First initialize WASM
import * as wasm_module from '../wasm/index'
import { wasm_test } from '../wasm/index'

// Initialize WASM before exporting
// await wasm_module.default()

const test = (): string => {
  return 'Hello, JS!'
}

export {
  wasm_test,
  test
}