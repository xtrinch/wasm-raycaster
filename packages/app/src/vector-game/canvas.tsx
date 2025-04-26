import { memo } from "react";

const Canvas = () => {
  return (
    <canvas
      id="display"
      style={{
        width: "100vw",
        height: "100vh",
        position: "absolute",
        left: 0,
        top: 0,
      }}
    ></canvas>
  );
};
export default memo(Canvas);
