import { test, wasm_test } from "@workspace/library";
import { observer } from "mobx-react";
import { useEffect } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";
import Map from "./vector-game/map/map";
import { GameContextProvider } from "./vector-game/state/gameContext";

const App = observer(() => {
  useEffect(() => {
    console.log(test());
    console.log(wasm_test());
  });

  return (
    <GameContextProvider>
      <div>
        <canvas
          id="display"
          width="1"
          height="1"
          style={{ width: "100%", height: "100%" }}
        ></canvas>
        <Map />
      </div>
    </GameContextProvider>
  );
});

const root = createRoot(document.getElementById("root"));
root.render(<App />);
