import { observer } from "mobx-react";
import Map from "./map/map";

const Game = observer(() => {
  return (
    <div>
      <canvas
        id="display"
        style={{
          width: "100%",
          height: "100%",
          position: "absolute",
          left: 0,
          top: 0,
        }}
      ></canvas>
      <Map />
    </div>
  );
});
export default Game;
