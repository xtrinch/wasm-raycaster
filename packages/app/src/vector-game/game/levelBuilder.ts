// Levels are written as placements at world x/y and added to a WasmWorld, which compiles
// them into its grid cells, sprites and model triangles (src/world.rs). Positions are world
// units (one cell is 1 by 1); grid geometry must use multiples of 0.1, the grid's precision.

import { TextureType, WasmWorld } from "../../../wasm";
import type { Color, Mesh } from "./objModel";

export type SegmentDirection = "east" | "north";
export type SegmentKind = "wall" | "door" | "window";

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
  mesh: number; // id from addMesh
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

// Uploads a parsed OBJ mesh once; model placements refer to it by the returned id.
export const addMesh = (world: WasmWorld, mesh: Mesh): number =>
  world.addMesh(
    new Float64Array(mesh.positions),
    new Uint32Array(mesh.triangles),
    new Float64Array(mesh.normals),
    // NaN marks triangles without a material colour; they use the placement colour.
    new Float32Array(mesh.colors.flatMap((color) => color ?? [NaN, NaN, NaN]))
  );

// Adds the placements in order; throws on the first one the grid cannot hold, such as a
// fourth segment in a cell or geometry off the 0.1 precision.
export function compileLevel(world: WasmWorld, placements: Placement[]): WasmWorld {
  for (const placement of placements) world.add(placement);
  return world;
}
