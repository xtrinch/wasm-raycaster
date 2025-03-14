import { Position } from "../game/player";

interface MapPersonProps {
  playerPosition: Position;
  size: number;
}

const MapPerson = (props: MapPersonProps) => {
  return (
    <div
      className="relative bg-red-500 w-[4px] h-[4px]"
      style={{
        top: `${props.playerPosition.y * 4 + 2}px`,
        left: `${props.playerPosition.x * 4 - 2}px`,
      }}
    ></div>
  );
};

export default MapPerson;
