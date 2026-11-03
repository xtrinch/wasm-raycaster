import bedMtl from "../../assets/models/bed.mtl?raw";
import bedObj from "../../assets/models/bed.obj?raw";
import chairMtl from "../../assets/models/chair.mtl?raw";
import chairObj from "../../assets/models/chair.obj?raw";
import peasantMtl from "../../assets/models/woman-peasant.mtl?raw";
import peasantObj from "../../assets/models/woman-peasant.obj?raw";
import rangerMtl from "../../assets/models/woman-ranger.mtl?raw";
import rangerObj from "../../assets/models/woman-ranger.obj?raw";
import tableMtl from "../../assets/models/table.mtl?raw";
import tableObj from "../../assets/models/table.obj?raw";
import { TextureType, WasmWorld } from "../../../wasm";
import { addMesh, area, compileLevel, door, model, pillar, sprite, wall, window } from "./levelBuilder";
import { parseMtl, parseObj } from "./objModel";

const peasantMesh = parseObj(peasantObj, parseMtl(peasantMtl));
const rangerMesh = parseObj(rangerObj, parseMtl(rangerMtl));
const bedMesh = parseObj(bedObj, parseMtl(bedMtl));
const chairMesh = parseObj(chairObj, parseMtl(chairMtl));
const tableMesh = parseObj(tableObj, parseMtl(tableMtl));
// Furniture files are in metres; this matches the women, 1.78 m tall at height 0.8.
const FURNITURE_SCALE = 0.45;

// The player starts at (4, 4) facing north (-y); see gameLoop.ts. Built once the WASM
// module has loaded (gameContext.tsx), since the world lives there.
export function buildLevel(): WasmWorld {
  const world = new WasmWorld(13);
  const peasant = addMesh(world, peasantMesh);
  const ranger = addMesh(world, rangerMesh);
  const bed = addMesh(world, bedMesh);
  const chair = addMesh(world, chairMesh);
  const table = addMesh(world, tableMesh);

  return compileLevel(world, [
    // Ground: road outside, floor and ceiling inside the building, nothing in row 12 past x 0.
    area({ x: 0, y: 0, w: 13, h: 12, road: true }),
    area({ x: 0, y: 12, w: 1, h: 1, road: true }),
    area({ x: 2, y: 2, w: 10, h: 8, floor: true, ceiling: true }),
    area({ x: 1, y: 3, w: 1, h: 1, floor: true, ceiling: true }),
    area({ x: 11, y: 2, w: 1, h: 1, road: true }),
    area({ x: 8, y: 5, w: 4, h: 2, road: true }),
    area({ x: 3, y: 9, w: 1, h: 1, road: true }),

    // Walls running along x (north)
    wall({ x: 2, y: 2, direction: "north", length: 1 }),
    window({ x: 3, y: 2, direction: "north", length: 1 }),
    wall({ x: 4, y: 2, direction: "north", length: 7.1 }),
    wall({ x: 1, y: 2.9, direction: "north", length: 1 }),
    wall({ x: 11, y: 3, direction: "north", length: 1 }),
    wall({ x: 1, y: 4, direction: "north", length: 1 }),
    wall({ x: 8, y: 4.9, direction: "north", length: 1 }),
    window({ x: 9, y: 4.9, direction: "north", length: 1, thickness: 0.1 }),
    wall({ x: 10, y: 4.9, direction: "north", length: 2 }),
    wall({ x: 8, y: 5.9, direction: "north", length: 1 }),
    wall({ x: 6, y: 6, direction: "north", length: 1.5 }),
    window({ x: 7.5, y: 6, direction: "north", length: 0.5 }),
    wall({ x: 8, y: 6.9, direction: "north", length: 3 }),
    window({ x: 11, y: 7, direction: "north", length: 0.9 }),
    door({ x: 6, y: 7.9, direction: "north", length: 1 }),
    wall({ x: 7, y: 7.9, direction: "north", length: 4 }),
    door({ x: 3, y: 9, direction: "north", length: 1 }),
    wall({ x: 2, y: 9.9, direction: "north", length: 1 }),
    window({ x: 4, y: 10, direction: "north", length: 1 }),
    wall({ x: 5, y: 9.9, direction: "north", length: 7 }),

    // Walls running along y (east)
    wall({ x: 2, y: 2, direction: "east", length: 1 }),
    door({ x: 1, y: 3, direction: "east", length: 1 }),
    window({ x: 2, y: 4.1, direction: "east", length: 2.9 }),
    wall({ x: 2, y: 7, direction: "east", length: 3 }),
    wall({ x: 2.9, y: 9, direction: "east", length: 1 }),
    window({ x: 4, y: 9.1, direction: "east", length: 0.9 }),
    wall({ x: 5.9, y: 6, direction: "east", length: 2 }),
    door({ x: 8, y: 5, direction: "east", length: 0.9 }),
    door({ x: 8, y: 6, direction: "east", length: 0.9 }),
    door({ x: 11, y: 2.1, direction: "east", length: 0.9 }),
    wall({ x: 11.9, y: 3, direction: "east", length: 2 }),
    wall({ x: 11.9, y: 7, direction: "east", length: 1 }),
    door({ x: 11.9, y: 8, direction: "east", length: 1 }),
    wall({ x: 11.9, y: 9, direction: "east", length: 1 }),

    // Pillar ahead of the player start
    pillar({ x: 4.2, y: 2.3, size: 0.2 }),

    // Sprites
    sprite({ x: 3, y: -1, height: 1, texture: TextureType.TREE_COLUMNAR }),
    sprite({ x: 1, y: 7, height: 1, texture: TextureType.TREE_COLUMNAR }),
    sprite({ x: -1, y: 5, height: 1.5, texture: TextureType.TREE_CONE }),
    sprite({ x: -2, y: 5, height: 0.5, texture: TextureType.TREE_CONE }),
    sprite({ x: -1, y: 6, height: 0.9, texture: TextureType.BUSH1 }),
    sprite({ x: -2, y: 4, height: 1, texture: TextureType.TREE_VASE }),
    model({ mesh: peasant, x: 2.4, y: 4.2, angle: 270, height: 0.8 }),
    model({ mesh: ranger, x: 0.8, y: 3.2, angle: 90, height: 0.8 }),
    // Bed against the west wall of the small room at x 6-8, y 6-8, headboard to the north.
    model({ mesh: bed, x: 6.23, y: 6.57, scale: FURNITURE_SCALE, solid: true }),
    // Table by the west window of the hall, with the chair facing it from the south.
    model({ mesh: table, x: 2.75, y: 5.7, scale: FURNITURE_SCALE, solid: true }),
    model({ mesh: chair, x: 2.75, y: 6.05, angle: 180, scale: FURNITURE_SCALE, solid: true }),
    sprite({ x: 4, y: 7, height: 0.7, texture: TextureType.PILLAR }),
    sprite({ x: 5, y: 5, height: 0.7, texture: TextureType.PILLAR }),
    sprite({ x: 14, y: 8.5, height: 0.5, texture: TextureType.PILLAR }),
    sprite({ x: -0.5, y: 1.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: -0.5, y: 3.5, height: 1, texture: TextureType.TREE_COLUMNAR }),
    sprite({ x: 18.5, y: 4.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 12.5, y: 5, height: 1, texture: TextureType.TREE_VASE }),
    sprite({ x: 12.5, y: 4.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 12.5, y: 12.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 3.5, y: 20.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 3.5, y: 14.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 14.5, y: 20.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 18.5, y: 10.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 18.5, y: 11.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 18.5, y: 12.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 21.5, y: 1.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 15.5, y: 0.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 16.0, y: 0.8, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 16.2, y: 0.2, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 9.5, y: 15.5, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 10.0, y: 15.1, height: 1, texture: TextureType.TREE_CONE }),
    sprite({ x: 10.5, y: 15.8, height: 1, texture: TextureType.TREE_CONE }),
  ]);
}
