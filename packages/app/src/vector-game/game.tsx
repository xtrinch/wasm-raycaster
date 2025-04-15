import { observer } from "mobx-react";
import Map from "./map/map";

const Game = observer(() => {
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
