/*
  Original author: Shaun Lebron
  Original article: https://observablehq.com/@shaunlebron/texture-drawing-for-html-canvas
*/

import { FILL_METHOD } from "./constants";
import fillQuadTex from "./fillQuadTex";

export { FILL_METHOD };

export const drawArbitraryQuadImage = (
  ctx: CanvasRenderingContext2D,
  texture: CanvasImageSource,
  src: {
    x: number;
    y: number;
  }[],
  dst: {
    x: number;
    y: number;
  }[],
  method = FILL_METHOD.BILINEAR,
  tesselation: number = 10
) => {
  const pattern = ctx.createPattern(texture, "no-repeat");

  ctx.fillStyle = pattern;

  fillQuadTex(ctx, src, dst, {
    tiles: tesselation,
    method,
  });
};
