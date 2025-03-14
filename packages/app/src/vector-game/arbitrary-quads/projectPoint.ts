import Matrix from "node-matrices";

export const projectPoint = (
  point: { x: number; y: number },
  projectionMatrix: Matrix
): { x: number; y: number } => {
  const pointMatrix = projectionMatrix.multiply(
    new Matrix([point.x], [point.y], [1])
  );

  return {
    x: pointMatrix.get(0, 0) / pointMatrix.get(2, 0),
    y: pointMatrix.get(1, 0) / pointMatrix.get(2, 0),
  };
};
