import { observer } from "mobx-react-lite";
import { useContext } from "react";
import { GameContext } from "../state/gameContext";
import MapDetail from "./mapDetail";
import MapPerson from "./mapPerson";

const Map = () => {
  const gameContext = useContext(GameContext);

  const map = gameContext.gameLoop?.map?.wallGrid;
  const size = gameContext.gameLoop?.map?.size;
  const playerPosition = gameContext.gameLoop?.player?.position;
  const fps = gameContext.gameLoop?.fps;
  const minFps = gameContext.gameLoop?.getMinFPS();

  if (!map || !playerPosition || !size) {
    return <></>;
  }
  return (
    <div
      className="absolute bottom-0 left-0 min-w-[100px]"
      style={{ width: size * 4 }}
    >
      <div className="text-white">{fps} FPS</div>
      <div className="text-white">{minFps} min FPS</div>
      <MapPerson playerPosition={{ ...playerPosition }} size={size} />
      <MapDetail map={map} size={size} />
    </div>
  );
};

export default observer(Map);
