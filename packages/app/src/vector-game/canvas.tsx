import { memo } from "react";

const Canvas = () => {
  return (
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
  );
};
export default memo(Canvas);
