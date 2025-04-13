import { observer } from "mobx-react";
import { useEffect } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";
import Game from "./vector-game/game";
import { GameContextProvider } from "./vector-game/state/gameContext";
import { test } from "./wasm";

const App = observer(() => {
  useEffect(() => {
    console.log(test());
  });

  return (
    <GameContextProvider>
      <Game />
    </GameContextProvider>
  );
});

const root = createRoot(document.getElementById("root"));
root.render(<App />);
