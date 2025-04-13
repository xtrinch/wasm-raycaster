/*
 * Copyright 2022 Google Inc. All Rights Reserved.
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *     http://www.apache.org/licenses/LICENSE-2.0
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { threads } from 'wasm-feature-detect';

const maxIterations = 1000;

const canvas = /** @type {HTMLCanvasElement} */ (
  document.getElementById('canvas')
);
const { width, height } = canvas;
const ctx = canvas.getContext('2d');
const timeOutput = /** @type {HTMLOutputElement} */ (
  document.getElementById('time')
);

function setupBtn(id, { generate }) {
  // Assign onclick handler + enable the button.
  Object.assign(document.getElementById(id), {
    async onclick() {
      const start = performance.now();
      const rawImageData = generate(width, height, maxIterations);
      const time = performance.now() - start;

      timeOutput.value = `${time.toFixed(2)} ms`;
      const imgData = new ImageData(rawImageData, width, height);
      ctx.putImageData(imgData, 0, 0);
    },
    disabled: false
  });
}

(async function initSingleThread() {
  const singleThread = await import('./pkg/wasm_bindgen_rayon_demo.js');
  await singleThread.default();
  setupBtn('singleThread', singleThread);
})();

(async function initMultiThread() {
  if (!(await threads())) return;
  const multiThread = await import('./pkg-parallel/wasm_bindgen_rayon_demo.js');
  await multiThread.default();
  await multiThread.initThreadPool(navigator.hardwareConcurrency);
  setupBtn('multiThread', multiThread);
})();
