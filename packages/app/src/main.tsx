import { observer } from "mobx-react";
import { createRoot } from "react-dom/client";
import "./style.css";
import Game from "./vector-game/game";
import { GameContextProvider } from "./vector-game/state/gameContext";

const App = observer(() => {
  return (
    <GameContextProvider>
      <Game />
    </GameContextProvider>
  );
});

const root = createRoot(document.getElementById("root"));
root.render(<App />);
