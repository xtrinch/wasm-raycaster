import { observer } from "mobx-react";
import { useContext, useEffect } from "react";
import { threads } from "wasm-feature-detect";
import Map from "./map/map";
import { GameContext } from "./state/gameContext";

const Game = observer(() => {
  const gameContext = useContext(GameContext);

  useEffect(() => {
    if (!gameContext.gameLoop) {
      return;
    }

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
