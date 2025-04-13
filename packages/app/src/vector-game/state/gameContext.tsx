import { makeAutoObservable } from "mobx";
import { createContext, useEffect, useMemo } from "react";
import { threads } from "wasm-feature-detect";
import def from "../../../wasm";
import { GameLoop } from "../game/gameLoop";
export class GameStore {
  public gameLoop: GameLoop;

  constructor() {
    makeAutoObservable(this);
  }

  public initialize = async () => {
    await def();
    if (await threads()) {
      const { initThreadPool } = await import("../../../wasm");
      await initThreadPool(navigator.hardwareConcurrency);
    }

    this.gameLoop = new GameLoop();
    this.gameLoop.start();
  };
}

const GameContext = createContext<GameStore>(null);

function GameContextProvider(props: any) {
  const gameStore = useMemo(() => new GameStore(), []);

  useEffect(() => {
    if (!gameStore.gameLoop) {
      gameStore.initialize();
    }
  }, []);

  return (
    <GameContext.Provider value={gameStore}>
      {props.children}
    </GameContext.Provider>
  );
}

export { GameContext, GameContextProvider };
