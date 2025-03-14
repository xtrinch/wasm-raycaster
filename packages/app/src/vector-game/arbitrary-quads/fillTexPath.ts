/*
  from: https://github.com/mrdoob/three.js/blob/r91/examples/js/renderers/CanvasRenderer.js#L917
  math: http://extremelysatisfactorytotalitarianism.com/blog/?p=2120
*/

/* eslint no-param-reassign:0 */

export const fillTexPath = (
  ctx: CanvasRenderingContext2D,
  x0: number,
  y0: number,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  u0: number,
  v0: number,
  u1: number,
  v1: number,
  u2: number,
  v2: number
): void => {
  x1 -= x0;
  y1 -= y0;
  x2 -= x0;
  y2 -= y0;

  u1 -= u0;
  v1 -= v0;
  u2 -= u0;
  v2 -= v0;

  const det = u1 * v2 - u2 * v1;

  if (det === 0) return;

  const idet = 1 / det;

  const m11 = (v2 * x1 - v1 * x2) * idet;
  const m12 = (v2 * y1 - v1 * y2) * idet;
  const m21 = (u1 * x2 - u2 * x1) * idet;
  const m22 = (u1 * y2 - u2 * y1) * idet;
  const dx = x0 - m11 * u0 - m21 * v0;
  const dy = y0 - m12 * u0 - m22 * v0;

  ctx.save();
  ctx.transform(m11, m12, m21, m22, dx, dy);
  ctx.fill();
  ctx.restore();
};
