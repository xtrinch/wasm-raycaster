// Render boxes are derived from directional cells in GridMap.wallGrid.
export type SurfaceBox = [number, number, number, number, number, number];
export type StairSection = "lower" | "upper";
export type StairDirection = "north" | "east" | "south" | "west";
export const STEP_COUNT = 12;
export const STAIR_HEIGHT = 1;
export const STEP_HEIGHT = STAIR_HEIGHT / STEP_COUNT;
export const STAIR_START_Y = 0.1;
export const STAIR_END_Y = 2.0;
export const STAIR_WIDTH = 0.9;
export const STAIR_CELL_BIT = 1n << 10n;
const STAIR_RUN = STAIR_END_Y - STAIR_START_Y;
const TREAD_DEPTH = STAIR_RUN / STEP_COUNT;
const DIRECTIONS: StairDirection[] = ["north", "east", "south", "west"];

// Each cell stores only its uphill direction and lower/upper half.
export function stairCell(facing: StairDirection, section: StairSection): bigint {
  const direction = DIRECTIONS.indexOf(facing);
  if (direction < 0 || (section !== "lower" && section !== "upper")) {
    throw new Error("Invalid stair direction or section");
  }
  return 8n | STAIR_CELL_BIT | (BigInt(direction) << 16n)
    | (section === "upper" ? 1n << 18n : 0n);
}

export function stairDescriptor(value: bigint) {
  return {
    facing: DIRECTIONS[Number((value >> 16n) & 3n)],
    section: (value & (1n << 18n) ? "upper" : "lower") as StairSection,
  };
}

export function stairHeightAt(grid: BigUint64Array, size: number, x: number, y: number): number | undefined {
  const cellX = Math.floor(x), cellY = Math.floor(y);
  if (cellX < 0 || cellY < 0 || cellX >= size || cellY >= size) return undefined;
  const value = grid[cellY * size + cellX];
  if (!(value & STAIR_CELL_BIT)) return undefined;
  const flight = stairDescriptor(value);
  const horizontal = flight.facing === "east" || flight.facing === "west";
  const lateral = horizontal ? y-cellY : x-cellX;
  const along = flight.facing === "north" ? 1-(y-cellY)
    : flight.facing === "west" ? 1-(x-cellX) : horizontal ? x-cellX : y-cellY;
  const start = STAIR_START_Y - (flight.section === "upper" ? 1 : 0);
  if (lateral >= STAIR_WIDTH || along < start || along >= start + STAIR_RUN) return undefined;
  for (let i = 0; i < STEP_COUNT; i++) {
    if (along < start + (i+1) * TREAD_DEPTH) {
      return (i+1) * STEP_HEIGHT;
    }
  }
  return undefined;
}

export function gridSupportHeight(grid: BigUint64Array, size: number, x: number, y: number, feet: number) {
  const stair = stairHeightAt(grid, size, x, y);
  if (stair !== undefined) return stair;
  const cellX = Math.floor(x), cellY = Math.floor(y);
  if (cellX < 0 || cellY < 0 || cellX >= size || cellY >= size) return 0;
  return feet >= 1 - STEP_HEIGHT - 0.001 && (grid[cellY * size + cellX] & 2n) ? 1 : 0;
}

export function buildStairBoxes(grid: BigUint64Array, size: number) {
  const pieces: SurfaceBox[] = [];
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    const value = grid[y*size+x];
    if (!(value & STAIR_CELL_BIT)) continue;
    const f = stairDescriptor(value);
    const start = STAIR_START_Y - (f.section === "upper" ? 1 : 0);
    for (let i = 0; i < STEP_COUNT; i++) {
      const a = Math.max(0, start + i*TREAD_DEPTH);
      const b = Math.min(1, start + (i+1)*TREAD_DEPTH);
      if (a >= b) continue;
      const low = 0, high = STAIR_WIDTH;
      const bounds = f.facing === "north" ? [x+low,y+1-b,x+high,y+1-a]
        : f.facing === "east" ? [x+a,y+low,x+b,y+high]
        : f.facing === "south" ? [x+low,y+a,x+high,y+b]
        : [x+1-b,y+low,x+1-a,y+high];
      pieces.push([...bounds,0,(i+1)*STEP_HEIGHT] as SurfaceBox);
    }
  }
  pieces.sort((a,b) => a[5]-b[5] || a[0]-b[0] || a[1]-b[1]);
  const stairs: SurfaceBox[] = [];
  const equal = (a: number,b: number) => Math.abs(a-b) < 1e-8;
  for (const piece of pieces) {
    const previous = stairs[stairs.length-1];
    if (previous && equal(previous[4],piece[4]) && equal(previous[5],piece[5])
      && equal(previous[0],piece[0]) && equal(previous[2],piece[2])
      && equal(previous[3],piece[1])) previous[3] = piece[3];
    else if (previous && equal(previous[4],piece[4]) && equal(previous[5],piece[5])
      && equal(previous[1],piece[1]) && equal(previous[3],piece[3])
      && equal(previous[2],piece[0])) previous[2] = piece[2];
    else stairs.push(piece);
  }
  return { stairs };
}

export function contains(box: SurfaceBox, x: number, y: number) {
  return x >= box[0] && x < box[2] && y >= box[1] && y < box[3];
}

