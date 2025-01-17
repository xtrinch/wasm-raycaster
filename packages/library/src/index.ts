import { wasm_test } from '../wasm/index'

const test = (): string => {
  return 'Hello from JS!'
}

export {
  wasm_test,
  test
}