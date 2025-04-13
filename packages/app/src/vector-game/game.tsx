import { observer } from "mobx-react";
import { useContext, useEffect } from "react";
import { threads } from "wasm-feature-detect";
import { test } from "../wasm";
import Map from "./map/map";
import { GameContext } from "./state/gameContext";

const Game = observer(() => {
  useEffect(() => {
    console.log(test());
  });

  const gameContext = useContext(GameContext);

  useEffect(() => {
    console.log(gameContext.gameLoop);
    if (!gameContext.gameLoop) {
      return;
    }
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

    const maxIterations = 1000;

    const canvas: HTMLCanvasElement =
      /** @type {HTMLCanvasElement} */ document.getElementById("canvas") as any;
    const { width, height } = canvas;
    const ctx = canvas.getContext("2d");
    const timeOutput: HTMLOutputElement =
      /** @type {HTMLOutputElement} */ document.getElementById("time") as any;

    const setupBtn = (id, { generate }) => {
      // Assign onclick handler + enable the button.
      Object.assign(document.getElementById(id), {
        async onclick() {
          console.log("click");
          const start = performance.now();
          const rawImageData = generate(width, height, maxIterations);
          const time = performance.now() - start;

          timeOutput.value = `${time.toFixed(2)} ms`;
          const imgData = new ImageData(rawImageData, width, height);
          ctx.putImageData(imgData, 0, 0);
        },
        disabled: false,
      });
    };

    (async function initSingleThread() {
      const def = await import("../../wasm-no-parallel");
      await def.default();

      setupBtn("singleThread", { generate: def.generate });
    })();

    console.log("will init mutli thread?");
    (async function initMultiThread() {
      if (!(await threads())) return;
      const { generate } = await import("../../wasm");

      setupBtn("multiThread", { generate });
    })();
  }, [gameContext.gameLoop]);

  return (
    <div>
      <canvas
        id="display"
        width="1"
        height="1"
        style={{ width: "100%", height: "100%" }}
      ></canvas>
      <Map />
    </div>
  );
});
export default Game;
