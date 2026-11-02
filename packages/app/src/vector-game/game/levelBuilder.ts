// Levels are written as placements at world x/y and compiled into GridMap.wallGrid
// cells plus the sprite list. Positions are world units (one cell is 1 by 1) and
// must be multiples of 0.1, the grid's precision.

import { TextureType } from "../../../wasm";
import { encodeCell, type Segment, type SegmentDirection, type SegmentKind } from "./mapBuilder";

export interface AreaPlacement {
  type: "area";
  x: number; // whole cells
  y: number;
  w: number;
  h: number;
  floor?: boolean;
  ceiling?: boolean;
  road?: boolean;
}

export interface SegmentPlacement {
  type: "segment";
  kind: SegmentKind;
  direction: SegmentDirection; // east runs along y, north runs along x
  x: number; // low corner; thickness extends towards +x (east) or +y (north)
  y: number;
  length: number;
  thickness: number;
}

export interface SpritePlacement {
  type: "sprite";
  x: number;
  y: number;
  texture: TextureType;
  height: number; // multiplier of one wall height
  angle: number; // 0-360
}

export type Placement = AreaPlacement | SegmentPlacement | SpritePlacement;

type AreaOptions = Omit<AreaPlacement, "type">;
type SegmentOptions = {
  x: number;
  y: number;
  direction: SegmentDirection;
  length: number;
  thickness?: number;
};

// Later areas replace the flags of earlier ones.
export const area = (options: AreaOptions): AreaPlacement => ({ type: "area", ...options });

const segment = (kind: SegmentKind, defaultThickness: number, options: SegmentOptions) => ({
  type: "segment" as const,
  kind,
  ...options,
  thickness: options.thickness ?? defaultThickness,
});

export const wall = (options: SegmentOptions): SegmentPlacement => segment("wall", 0.1, options);
export const door = (options: SegmentOptions): SegmentPlacement => segment("door", 0.1, options);
export const window = (options: SegmentOptions): SegmentPlacement => segment("window", 0, options);

// A square block of wall with its low corner at x/y.
export const pillar = (options: { x: number; y: number; size: number }): SegmentPlacement =>
  segment("wall", options.size, {
    x: options.x,
    y: options.y,
    direction: "east",
    length: options.size,
  });

export const sprite = (options: Omit<SpritePlacement, "type" | "angle"> & { angle?: number }): SpritePlacement => ({
  type: "sprite",
  angle: 0,
  ...options,
});

export interface Level {
  size: number;
  wallGrid: BigUint64Array;
  sprites: number[][]; // x, y, angle, height, texture, as SpriteMap expects
}

const tenths = (value: number, name: string): number => {
  const scaled = Math.round(value * 10);
  if (Math.abs(value * 10 - scaled) > 1e-6) {
    throw new Error(`${name} ${value} is not a multiple of 0.1`);
  }
  return scaled;
};

// Clip [start, start + size) in tenths to each cell it touches.
function splitRange(start: number, size: number) {
  const pieces: { cell: number; offset: number; size: number }[] = [];
  for (let cell = Math.floor(start / 10); cell * 10 < start + size; cell++) {
    const from = Math.max(start, cell * 10);
    const to = Math.min(start + size, cell * 10 + 10);
    pieces.push({ cell, offset: from - cell * 10, size: to - from });
  }
  return pieces;
}

const permutations = <T>(items: T[]): T[][] =>
  items.length <= 1
    ? [items]
    : items.flatMap((item, i) =>
        permutations([...items.slice(0, i), ...items.slice(i + 1)]).map((rest) => [item, ...rest])
      );

export function compileLevel(size: number, placements: Placement[]): Level {
  const flags = Array.from({ length: size * size }, () => ({ floor: false, ceiling: false, road: false }));
  const segments: Segment[][] = Array.from({ length: size * size }, () => []);
  const sprites: number[][] = [];

  const index = (x: number, y: number, what: string) => {
    if (x < 0 || y < 0 || x >= size || y >= size) {
      throw new Error(`${what} reaches cell (${x}, ${y}) outside the ${size}x${size} grid`);
    }
    return y * size + x;
  };

  for (const p of placements) {
    if (p.type === "sprite") {
      sprites.push([p.x, p.y, p.angle, p.height, p.texture]);
    } else if (p.type === "area") {
      for (let y = p.y; y < p.y + p.h; y++) {
        for (let x = p.x; x < p.x + p.w; x++) {
          flags[index(x, y, "area")] = { floor: !!p.floor, ceiling: !!p.ceiling, road: !!p.road };
        }
      }
    } else {
      const what = `${p.kind} at (${p.x}, ${p.y})`;
      const east = p.direction === "east";
      const across = tenths(east ? p.x : p.y, `${what} position`);
      const along = tenths(east ? p.y : p.x, `${what} position`);
      const length = tenths(p.length, `${what} length`);
      const thickness = tenths(p.thickness, `${what} thickness`);
      if (length <= 0) throw new Error(`${what} needs a positive length`);

      // A flat segment on a cell line goes into the cell after the line.
      const acrossPieces =
        thickness > 0
          ? splitRange(across, thickness)
          : [{ cell: Math.floor(across / 10), offset: across % 10, size: 0 }];

      for (const a of acrossPieces) {
        for (const l of splitRange(along, length)) {
          const cell = east ? index(a.cell, l.cell, what) : index(l.cell, a.cell, what);
          segments[cell].push({
            kind: p.kind,
            direction: p.direction,
            offset: a.offset,
            thickness: a.size,
            start: l.offset,
            length: l.size,
          });
        }
      }
    }
  }

  const wallGrid = new BigUint64Array(size * size);
  for (let i = 0; i < size * size; i++) {
    const where = `cell (${i % size}, ${Math.floor(i / size)})`;
    if (segments[i].length > 3) {
      throw new Error(`${where} needs ${segments[i].length} segments; a cell holds at most 3`);
    }
    // Slot order matters for windows and the shared door bit, so try each order.
    let encoded: bigint | undefined;
    let lastError: unknown;
    for (const order of permutations(segments[i])) {
      try {
        encoded = encodeCell({ ...flags[i], segments: order });
        break;
      } catch (error) {
        lastError = error;
      }
    }
    if (encoded === undefined) throw new Error(`${where}: ${(lastError as Error).message}`);
    wallGrid[i] = encoded;
  }

  return { size, wallGrid, sprites };
}
