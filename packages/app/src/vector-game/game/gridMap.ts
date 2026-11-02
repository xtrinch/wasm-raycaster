import { makeAutoObservable } from "mobx";
import pillarTexture from "../../assets/barrel1.png";
import bush1Texture from "../../assets/bushes/Bushes1/Bush1_1.png";
import ceilingTexture from "../../assets/ceiling-scaled2.jpeg";
import panorama from "../../assets/deathvalley_panorama.jpg";
import doorTexture from "../../assets/door3.png";
import floorTexture3 from "../../assets/floor5-scaled.jpeg";
import roadTexture from "../../assets/gravel.jpeg";
import treeTextureColumnar from "../../assets/trees/columnnar.png";
import treeTexture from "../../assets/trees/pyramid.png";
import treeTextureVase from "../../assets/trees/vase.png";
import wallTexture from "../../assets/wall_texture.jpg";
import windowTexture from "../../assets/window1.png";
import lady1Texture from "../../assets/woman/woman1.png";
import lady2Texture from "../../assets/woman/woman2.png";
import lady3Texture from "../../assets/woman/woman3.png";
import lady4Texture from "../../assets/woman/woman4.png";
import lady5Texture from "../../assets/woman/woman5.png";
import lady6Texture from "../../assets/woman/woman6.png";
import lady7Texture from "../../assets/woman/woman7.png";
import lady8Texture from "../../assets/woman/woman8.png";

import { TextureType } from "../../../wasm";
import { Bitmap } from "./bitmap";
import { cell, door, wall, window } from "./mapBuilder";
import roofConcrete from "../../assets/roof-concrete.jpg";

export interface Point {
  x: number; // x coordinate on the grid
  y: number; // y coordinate on the grid
  flooredX?: number; // to know exactly which coordinate in the grid we checked in
  flooredY?: number; // to know exactly which coordinate in the grid we checked in
  height?: number;
  distance?: number;
  shading?: number;
  offset?: number;
  length2?: number;
  type?: "wall" | "tree"; // whether there is a wall or a tree at a certain point on the grid
}

export class GridMap {
  public size: number;
  public wallGrid: BigUint64Array;
  public skybox: Bitmap;
  public wallTexture: Bitmap;
  public treeTexture: Bitmap;
  public floorTexture: Bitmap;
  public ceilingTexture: Bitmap;
  public roadTexture: Bitmap;
  public treeTextureVase: Bitmap;
  public treeTextureColumnar: Bitmap;
  public pillarTexture: Bitmap;
  public bush1Texture: Bitmap;
  public doorTexture: Bitmap;
  public windowTexture: Bitmap;
  public ladyTextures: Bitmap[];
  public light: number;
  public roofTexture: Bitmap;

  constructor(size: number) {
    this.size = size;
    this.roofTexture = new Bitmap(roofConcrete, 256, 256);
    this.wallGrid = new BigUint64Array(size * size);
    this.skybox = new Bitmap(panorama, 2000, 750);
    this.windowTexture = new Bitmap(windowTexture, 1024, 1024);
    this.wallTexture = new Bitmap(wallTexture, 1024, 1024);
    this.doorTexture = new Bitmap(doorTexture, 1024, 1024); // these two should be equal width / height for simplicity sake
    this.roadTexture = new Bitmap(roadTexture, 874, 874);
    this.treeTexture = new Bitmap(treeTexture, 452, 679);
    this.treeTextureVase = new Bitmap(treeTextureVase, 500, 522);
    this.floorTexture = new Bitmap(floorTexture3, 187, 187);
    this.ceilingTexture = new Bitmap(ceilingTexture, 145, 145);
    this.treeTextureColumnar = new Bitmap(treeTextureColumnar, 229, 645);
    this.pillarTexture = new Bitmap(pillarTexture, 355, 438);
    this.bush1Texture = new Bitmap(bush1Texture, 102, 89);
    this.ladyTextures = [
      new Bitmap(lady1Texture, 320, 632),
      new Bitmap(lady2Texture, 320, 632),
      new Bitmap(lady3Texture, 320, 632),
      new Bitmap(lady4Texture, 320, 632),
      new Bitmap(lady5Texture, 320, 632),
      new Bitmap(lady6Texture, 320, 632),
      new Bitmap(lady7Texture, 320, 632),
      new Bitmap(lady8Texture, 320, 632),
    ];
    this.light = 0;

    // Each cell is one u64, row-major (index = y * size + x). Decoded in src/lib.rs.
    //
    // Flags (bits 0-11):
    //   bit 0     - cell has segments (minimap colour); exactly 0x1 is a solid full-cell block
    //   bit 1     - floor
    //   bit 2     - ceiling underside and roof top
    //   bit 3     - road
    //   bit 4     - segment slots 1 and 2 are doors (one shared bit)
    //   bit 5     - segment slot 0 is a door
    //   bits 6-8  - segment slot 0/1/2 direction: clear = east (runs along y),
    //               set = north (runs along x)
    //   bits 9-10 - segment slot 0/1 is a window (slot 2 cannot be a window)
    //   bit 11    - reserved
    //
    // Segment count and geometry (bits 12-63):
    //   bits 12-15 - number of segment slots used (0-3)
    //   bits 16-31 - slot 0, bits 32-47 - slot 1, bits 48-63 - slot 2
    //   each slot holds four 4-bit fields, low to high, in tenths of a cell (0-10):
    //     offset    - position across the segment, from the cell's low x/y edge
    //     thickness - size across the segment
    //     length    - size along the segment
    //     start     - position along the segment, from the cell's low x/y edge
    //
    // Segment kinds: a slot is a plain wall unless its door bit (5, or shared 4) or its
    // window bit (9/10) is set. Doors and windows use the slot's geometry above, and
    // the door bit wins if both are set. Only slots 0 and 1 can be windows.
    //
    // Examples:
    //   0x0000000000000008 - road only
    //   0x0000000000000006 - floor and ceiling (indoors)
    //   0x000000000A101057 - indoors, 1 north wall: offset 0, thickness 0.1, full length
    //   0x000000000A101037 - indoors, 1 east door: offset 0, thickness 0.1, full length
    //   0x000000000A001217 - indoors, 1 east window: offset 0, thickness 0, full length
    //
    // Cells are written with cell() from mapBuilder.ts, which packs these bits;
    // cell({ segments }) stores segments[i] in slot i.

    // prettier-ignore
    this.wallGrid = new BigUint64Array([
      // row 0
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ road: true }),
      /*  3 */ cell({ road: true }),
      /*  4 */ cell({ road: true }),
      /*  5 */ cell({ road: true }),
      /*  6 */ cell({ road: true }),
      /*  7 */ cell({ road: true }),
      /*  8 */ cell({ road: true }),
      /*  9 */ cell({ road: true }),
      /* 10 */ cell({ road: true }),
      /* 11 */ cell({ road: true }),
      /* 12 */ cell({ road: true }),
      // row 1
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ road: true }),
      /*  3 */ cell({ road: true }),
      /*  4 */ cell({ road: true }),
      /*  5 */ cell({ road: true }),
      /*  6 */ cell({ road: true }),
      /*  7 */ cell({ road: true }),
      /*  8 */ cell({ road: true }),
      /*  9 */ cell({ road: true }),
      /* 10 */ cell({ road: true }),
      /* 11 */ cell({ road: true }),
      /* 12 */ cell({ road: true }),
      // row 2
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east" }), wall({ direction: "north" })] }),
      /*  3 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "north" })] }),
      /*  4 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" }), wall({ direction: "east", offset: 2, thickness: 2, start: 3, length: 2 })] }), // CHANGED: pillar (one thin wall, segment 1), x 4.2-4.4, y 2.3-2.5, 1.5 in front of and 0.2 left of player start
      /*  5 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /*  6 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /*  7 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /*  8 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /*  9 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /* 10 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /* 11 */ cell({ road: true, segments: [door({ direction: "east", thickness: 1, length: 9, start: 1 }), wall({ direction: "north", length: 1 })] }),
      /* 12 */ cell({ road: true }),
      // row 3
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ floor: true, ceiling: true, segments: [door({ direction: "east", thickness: 1 })] }),
      /*  2 */ cell({ floor: true, ceiling: true }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }),
      /*  5 */ cell({ floor: true, ceiling: true }),
      /*  6 */ cell({ floor: true, ceiling: true }),
      /*  7 */ cell({ floor: true, ceiling: true }),
      /*  8 */ cell({ floor: true, ceiling: true }),
      /*  9 */ cell({ floor: true, ceiling: true }),
      /* 10 */ cell({ floor: true, ceiling: true }),
      /* 11 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 }), wall({ direction: "north" })] }),
      /* 12 */ cell({ road: true }),
      // row 4
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true, segments: [wall({ direction: "north" })] }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "east", length: 9, start: 1 })] }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }), // player start (4, 4) facing north, see gameLoop.ts
      /*  5 */ cell({ floor: true, ceiling: true }),
      /*  6 */ cell({ floor: true, ceiling: true }),
      /*  7 */ cell({ floor: true, ceiling: true }),
      /*  8 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  9 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "north", offset: 9, thickness: 1 })] }),
      /* 10 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 11 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 }), wall({ direction: "north", offset: 9 })] }),
      /* 12 */ cell({ road: true }),
      // row 5
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "east" })] }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }),
      /*  5 */ cell({ floor: true, ceiling: true }),
      /*  6 */ cell({ floor: true, ceiling: true }),
      /*  7 */ cell({ floor: true, ceiling: true }),
      /*  8 */ cell({ road: true, segments: [wall({ direction: "north", offset: 9 }), door({ direction: "east", thickness: 1, length: 9 })] }),
      /*  9 */ cell({ road: true }),
      /* 10 */ cell({ road: true }),
      /* 11 */ cell({ road: true }),
      /* 12 */ cell({ road: true }),
      // row 6
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "east" })] }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }),
      /*  5 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 })] }),
      /*  6 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north" })] }),
      /*  7 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "north", length: 5, start: 5 }), wall({ direction: "north", length: 5 })] }),
      /*  8 */ cell({ road: true, segments: [wall({ direction: "north", offset: 9 }), door({ direction: "east", thickness: 1, length: 9 })] }),
      /*  9 */ cell({ road: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 10 */ cell({ road: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 11 */ cell({ road: true, segments: [window({ direction: "north", offset: 10, length: 9 })] }),
      /* 12 */ cell({ road: true }),
      // row 7
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east" })] }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }),
      /*  5 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 })] }),
      /*  6 */ cell({ floor: true, ceiling: true, segments: [door({ direction: "north", thickness: 1, offset: 9 })] }),
      /*  7 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  8 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  9 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 10 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 11 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 })] }),
      /* 12 */ cell({ road: true }),
      // row 8
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east" })] }),
      /*  3 */ cell({ floor: true, ceiling: true }),
      /*  4 */ cell({ floor: true, ceiling: true }),
      /*  5 */ cell({ floor: true, ceiling: true }),
      /*  6 */ cell({ floor: true, ceiling: true }),
      /*  7 */ cell({ floor: true, ceiling: true }),
      /*  8 */ cell({ floor: true, ceiling: true }),
      /*  9 */ cell({ floor: true, ceiling: true }),
      /* 10 */ cell({ floor: true, ceiling: true }),
      /* 11 */ cell({ floor: true, ceiling: true, segments: [door({ direction: "east", thickness: 1, offset: 9 })] }),
      /* 12 */ cell({ road: true }),
      // row 9
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east" }), wall({ direction: "north", offset: 9 }), wall({ direction: "east", offset: 9 })] }),
      /*  3 */ cell({ road: true, segments: [door({ direction: "north", thickness: 1 })] }),
      /*  4 */ cell({ floor: true, ceiling: true, segments: [window({ direction: "north", offset: 10 }), window({ direction: "east", length: 9, start: 1 })] }),
      /*  5 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  6 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  7 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  8 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /*  9 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 10 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "north", offset: 9 })] }),
      /* 11 */ cell({ floor: true, ceiling: true, segments: [wall({ direction: "east", offset: 9 }), wall({ direction: "north", offset: 9 })] }),
      /* 12 */ cell({ road: true }),
      // row 10
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ road: true }),
      /*  3 */ cell({ road: true }),
      /*  4 */ cell({ road: true }),
      /*  5 */ cell({ road: true }),
      /*  6 */ cell({ road: true }),
      /*  7 */ cell({ road: true }),
      /*  8 */ cell({ road: true }),
      /*  9 */ cell({ road: true }),
      /* 10 */ cell({ road: true }),
      /* 11 */ cell({ road: true }),
      /* 12 */ cell({ road: true }),
      // row 11
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({ road: true }),
      /*  2 */ cell({ road: true }),
      /*  3 */ cell({ road: true }),
      /*  4 */ cell({ road: true }),
      /*  5 */ cell({ road: true }),
      /*  6 */ cell({ road: true }),
      /*  7 */ cell({ road: true }),
      /*  8 */ cell({ road: true }),
      /*  9 */ cell({ road: true }),
      /* 10 */ cell({ road: true }),
      /* 11 */ cell({ road: true }),
      /* 12 */ cell({ road: true }),
      // row 12
      /*  0 */ cell({ road: true }),
      /*  1 */ cell({}),
      /*  2 */ cell({}),
      /*  3 */ cell({}),
      /*  4 */ cell({}),
      /*  5 */ cell({}),
      /*  6 */ cell({}),
      /*  7 */ cell({}),
      /*  8 */ cell({}),
      /*  9 */ cell({}),
      /* 10 */ cell({}),
      /* 11 */ cell({}),
      /* 12 */ cell({}),
    ]);
    this.size = Math.sqrt(this.wallGrid.length);

    makeAutoObservable(this);
  }

  hash(x: number, y: number) {
    return ((x * 73856093) ^ (y * 19349663)) % 100;
  }

  public update = (seconds: number) => {
    if (this.light > 0) this.light = Math.max(this.light - 10 * seconds, 0);
    // else if (Math.random() * 5 < seconds) this.light = 2;
  };

  getSpriteTextureArray(): Int32Array {
    return new Int32Array([
      TextureType.WALL,
      this.getSpriteTexture(TextureType.WALL).texture.height,
      this.getSpriteTexture(TextureType.WALL).texture.width,
      this.getSpriteData(TextureType.WALL).angles,
      TextureType.CEILING,
      this.getSpriteTexture(TextureType.CEILING).texture.height,
      this.getSpriteTexture(TextureType.CEILING).texture.width,
      this.getSpriteData(TextureType.CEILING).angles,
      TextureType.FLOOR,
      this.getSpriteTexture(TextureType.FLOOR).texture.height,
      this.getSpriteTexture(TextureType.FLOOR).texture.width,
      this.getSpriteData(TextureType.FLOOR).angles,
      TextureType.ROAD,
      this.getSpriteTexture(TextureType.ROAD).texture.height,
      this.getSpriteTexture(TextureType.ROAD).texture.width,
      this.getSpriteData(TextureType.ROAD).angles,
      TextureType.DOOR,
      this.getSpriteTexture(TextureType.DOOR).texture.height,
      this.getSpriteTexture(TextureType.DOOR).texture.width,
      this.getSpriteData(TextureType.DOOR).angles,
      TextureType.TREE_CONE,
      this.getSpriteTexture(TextureType.TREE_CONE).texture.height,
      this.getSpriteTexture(TextureType.TREE_CONE).texture.width,
      this.getSpriteData(TextureType.TREE_CONE).angles,
      TextureType.PILLAR,
      this.getSpriteTexture(TextureType.PILLAR).texture.height,
      this.getSpriteTexture(TextureType.PILLAR).texture.width,
      this.getSpriteData(TextureType.PILLAR).angles,
      TextureType.BUSH1,
      this.getSpriteTexture(TextureType.BUSH1).texture.height,
      this.getSpriteTexture(TextureType.BUSH1).texture.width,
      this.getSpriteData(TextureType.BUSH1).angles,
      TextureType.TREE_VASE,
      this.getSpriteTexture(TextureType.TREE_VASE).texture.height,
      this.getSpriteTexture(TextureType.TREE_VASE).texture.width,
      this.getSpriteData(TextureType.TREE_VASE).angles,
      TextureType.TREE_COLUMNAR,
      this.getSpriteTexture(TextureType.TREE_COLUMNAR).texture.height,
      this.getSpriteTexture(TextureType.TREE_COLUMNAR).texture.width,
      this.getSpriteData(TextureType.TREE_COLUMNAR).angles,
      TextureType.LADY,
      this.getSpriteTexture(TextureType.LADY).texture.height,
      this.getSpriteTexture(TextureType.LADY).texture.width,
      this.getSpriteData(TextureType.LADY).angles,
      TextureType.WINDOW,
      this.getSpriteTexture(TextureType.WINDOW).texture.height,
      this.getSpriteTexture(TextureType.WINDOW).texture.width,
      this.getSpriteData(TextureType.WINDOW).angles,
    ]);
  }

  public getSpriteTexture = (
    spriteType: TextureType,
    angleVal: number = 0
  ): { texture: Bitmap } => {
    let texture: Bitmap;

    switch (spriteType) {
      case TextureType.ROOF:
        texture = this.roofTexture;
        break;
      case TextureType.WALL:
        texture = this.wallTexture;
        break;
      case TextureType.CEILING:
        texture = this.ceilingTexture;
        break;
      case TextureType.FLOOR:
        texture = this.floorTexture;
        break;
      case TextureType.ROAD:
        texture = this.roadTexture;
        break;
      case TextureType.DOOR:
        texture = this.doorTexture;
        break;
      case TextureType.LADY:
        texture = this.ladyTextures[angleVal];
        break;
      case TextureType.TREE_CONE:
        texture = this.treeTexture;
        break;
      case TextureType.TREE_VASE:
        texture = this.treeTextureVase;
        break;
      case TextureType.TREE_COLUMNAR:
        texture = this.treeTextureColumnar;
        break;
      case TextureType.PILLAR:
        texture = this.pillarTexture;
        break;
      case TextureType.BUSH1:
        texture = this.bush1Texture;
        break;
      case TextureType.WINDOW:
        texture = this.windowTexture;
        break;
      default:
        console.log("Sprite texture not found for type " + spriteType);
        texture = this.treeTexture;
        break;
    }

    return { texture };
  };

  public getSpriteData = (spriteType: TextureType): { angles: number } => {
    let angles = 1;

    switch (spriteType) {
      case TextureType.ROOF:
      case TextureType.WALL:
      case TextureType.CEILING:
      case TextureType.FLOOR:
      case TextureType.ROAD:
      case TextureType.DOOR:
        break;
      case TextureType.LADY:
        angles = 8;
        break;
      case TextureType.TREE_CONE:
        break;
      case TextureType.TREE_VASE:
        break;
      case TextureType.TREE_COLUMNAR:
        break;
      case TextureType.PILLAR:
        break;
      case TextureType.BUSH1:
        break;
      case TextureType.WINDOW:
        break;
      default:
        console.log("Sprite texture not found for type " + spriteType);
        break;
    }

    return { angles };
  };
}
