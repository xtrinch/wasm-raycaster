import { Position } from "../game/player";

interface MapDetailProps {
  map: Uint8Array;
  size: number;
  playerPosition: Position;
}

const MapDetail = (props: MapDetailProps) => {
  return (
    <div className="flex flex-row flex-wrap" style={{ width: props.size * 4 }}>
      {[...props.map].map((pix, idx) => {
        return (
          <div
            key={idx}
            className={`w-[4px] h-[4px] ${
              pix == 1 ? "bg-gray-500" : "bg-black"
            } ${pix == 2 ? "bg-green-500" : "bg-black"}`}
          />
        );
      })}
    </div>
  );
};

export default MapDetail;
