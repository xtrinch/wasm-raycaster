// Levels are written as placements at world x/y and compiled into GridMap.wallGrid
// cells, the sprite list and mesh triangles. Positions are world units (one cell is
// 1 by 1); grid geometry must use multiples of 0.1, the grid's precision.

import { TextureType } from "../../../wasm";
import { encodeCell, type Segment, type SegmentDirection, type SegmentKind } from "./mapBuilder";
import type { Color, Mesh } from "./objModel";

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

// A 3D model; it blocks movement only when solid.
export interface ModelPlacement {
  type: "model";
  mesh: Mesh;
  x: number; // centre of the model's footprint
  y: number;
  z: number; // height of the model's lowest point
  scale: number; // ignored when height is set
  height?: number; // scale the model to exactly this tall, in world units
  angle: number; // degrees around the vertical axis; 0 faces the file's front (+Z) towards +y
  color: Color; // 0-1, for triangles without a material colour
  solid: boolean; // block movement with the model's bounding box; feet can stand on its top
}

export type Placement = AreaPlacement | SegmentPlacement | SpritePlacement | ModelPlacement;

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

export const model = (
  options: Omit<ModelPlacement, "type" | "z" | "scale" | "angle" | "color" | "solid"> &
    Partial<Pick<ModelPlacement, "z" | "scale" | "angle" | "color" | "solid">>
): ModelPlacement => ({
  type: "model",
  z: 0,
  scale: 1,
  angle: 0,
  color: [0.8, 0.8, 0.8],
  solid: false,
  ...options,
});

// Floats per mesh triangle, matching MESH_TRIANGLE_STRIDE in lib.rs.
const MESH_TRIANGLE_STRIDE = 21;

export interface Level {
  size: number;
  wallGrid: BigUint64Array;
  sprites: number[][]; // x, y, angle, height, texture, as SpriteMap expects
  meshTriangles: Float32Array; // per vertex world x, y, z and normal, then r, g, b; see lib.rs
  // Per model: first triangle, triangle count, footprint min x, min y, max x, max y.
  models: Float32Array;
  // Per solid model: bounding box min x, min y, max x, max y, then top height.
  solids: Float32Array;
}

// OBJ files are Y up; the world is Z up with the ground on x/y.
function bakeModel(p: ModelPlacement, out: number[]): { footprint: number[]; top: number } {
  const { positions, triangles, normals } = p.mesh;
  const cos = Math.cos((p.angle * Math.PI) / 180);
  const sin = Math.sin((p.angle * Math.PI) / 180);
  // Swap the file's Y-up axes to the world's Z-up, then turn around the vertical axis.
  const toWorld = (x: number, y: number, z: number) => [x * cos - z * sin, x * sin + z * cos, y];

  const world: number[] = [];
  for (let i = 0; i < positions.length; i += 3) {
    world.push(...toWorld(positions[i], positions[i + 1], positions[i + 2]));
  }
  const min = [Infinity, Infinity, Infinity];
  const max = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < world.length; i++) {
    min[i % 3] = Math.min(min[i % 3], world[i]);
    max[i % 3] = Math.max(max[i % 3], world[i]);
  }
  const scale = p.height !== undefined ? p.height / (max[2] - min[2]) : p.scale;
  // Centre the footprint on x/y and stand the lowest point on z.
  const shift = [p.x - ((min[0] + max[0]) / 2) * scale, p.y - ((min[1] + max[1]) / 2) * scale, p.z - min[2] * scale];
  for (let t = 0; t < triangles.length; t += 3) {
    for (let corner = t; corner < t + 3; corner++) {
      const vertex = triangles[corner] * 3;
      const n = toWorld(normals[corner * 3], normals[corner * 3 + 1], normals[corner * 3 + 2]);
      out.push(
        world[vertex] * scale + shift[0],
        world[vertex + 1] * scale + shift[1],
        world[vertex + 2] * scale + shift[2],
        ...n
      );
    }
    out.push(...(p.mesh.colors[t / 3] ?? p.color));
  }
  // The placed footprint, for registering the model in the cells it covers.
  const halfWidth = ((max[0] - min[0]) / 2) * scale;
  const halfDepth = ((max[1] - min[1]) / 2) * scale;
  return {
    footprint: [p.x - halfWidth, p.y - halfDepth, p.x + halfWidth, p.y + halfDepth],
    top: p.z + (max[2] - min[2]) * scale,
  };
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
  const meshTriangles: number[] = [];
  const models: number[] = [];
  const solids: number[] = [];

  const index = (x: number, y: number, what: string) => {
    if (x < 0 || y < 0 || x >= size || y >= size) {
      throw new Error(`${what} reaches cell (${x}, ${y}) outside the ${size}x${size} grid`);
    }
    return y * size + x;
  };

  for (const p of placements) {
    if (p.type === "model") {
      const first = meshTriangles.length / MESH_TRIANGLE_STRIDE;
      const { footprint, top } = bakeModel(p, meshTriangles);
      models.push(first, meshTriangles.length / MESH_TRIANGLE_STRIDE - first, ...footprint);
      if (p.solid) solids.push(...footprint, top);
    } else if (p.type === "sprite") {
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

  return {
    size,
    wallGrid,
    sprites,
    meshTriangles: new Float32Array(meshTriangles),
    models: new Float32Array(models),
    solids: new Float32Array(solids),
  };
}
