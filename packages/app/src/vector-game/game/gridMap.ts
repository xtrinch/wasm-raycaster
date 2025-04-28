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

import { Bitmap } from "./bitmap";
import { SpriteType } from "./spriteMap";

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

  constructor(size: number) {
    this.size = size;
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

    // 64 bits, 8 bytes
    //     offset_secondary3  width3     thickness3 offset_primary3    offset_secondary2  width2     thickness2 offset_primary2    offset_secondary  width     thickness offset_primary   reserve1 reserve2 num_walls1 num_walls0 reserve5 reserve6 reserve7 is_window   north2  north  door  door23   road   north3 ceiling&floor  wall
    // BIN 0000               0000       0000       0000               0000               0000       0000       0000               0000              0000      0000      0000             0        0        0          0          0        0        0        0           0       0      0     0        0      0       0              0

    // - offset primary would be from east / north, and secondary for east would be north, and for north it will be east
    // 0x1 - wall (bit 0)
    // 0x2 - floor (bit 1)
    // 0x4 - ceiling (bit 2)
    // 0x6 - ceiling / floor (bit 1, 2)
    // 0x8 - road (bit 3)
    // 0x11 - thin wall (bit 0, 4)
    // 0x51 - thin wall - north (bit 0, 4, 6)
    // 0x57 - thin wall with ceiling / floor - north (bit 0, 1, 2, 4, 6)
    // 0x17 - thin wall with ceiling / floor (bit 0, 1, 2, 4)
    // 0x19 - thin wall with road (bit 0, 3, 4)
    // 0x59 - thin wall with road - north (bit 0, 3, 4, 6)
    // 0x31 - thin door - east (bit 0, 4, 5)
    // 0x39 - thin door - east with road (bit 0, 3, 4, 5)
    // 0x37 - thin door - east with wall & floor (bit 0, 1, 2, 4, 5)
    // 0x71 - thin door - north (bit 0, 4, 5, 6)

    // prettier-ignore
    this.wallGrid = new BigUint64Array([
      /*       0,            1,            2,            3,            4,            5,            6,            7,            8,            9,            10,           11           */
      /* 0  */ 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008,
      /* 1  */ 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008,
      /* 2  */ 0x0000000000000008, 0x000000000A191059, 0x00000A100A102087, 0x000000000A001157, 0x000000000A101057, 0x000000000A101057, 0x000000000A101057, 0x000000000A101057, 0x000000000A101057, 0x000000000A101057, 0x000000000A101057, 0x00000110191020A9, 0x0000000000000008,
      /* 3  */ 0x0000000000000008, 0x000000000A101037, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x00000A100A192087, 0x0000000000000008,
      /* 4  */ 0x0000000000000008, 0x000000000A101059, 0x0000000019001117, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x000000000A191057, 0x000000000A191157, 0x000000000A191057, 0x00000A190A192087, 0x0000000000000008,
      /* 5  */ 0x0000000000000008, 0x0000000000000008, 0x000000000A001117, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x000009100A192059, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008,
      /* 6  */ 0x0000000000000008, 0x0000000000000008, 0x000000000A001117, 0x0000000000000006, 0x0000000000000006, 0x000000000A191017, 0x000000000A101057, 0x00000510550021C7, 0x0000000000000001, 0x0000000000000008, 0x0000000000000008, 0x00000000090A1159, 0x0000000000000008,
      /* 7  */ 0x0000000000000008, 0x0000000000000008, 0x000000000A101017, 0x0000000000000006, 0x0000000000000006, 0x000000000A191017, 0x000000000A191077, 0x000000000A191057, 0x0000000000000001, 0x0000000000000001, 0x0000000000000001, 0x000000000A191017, 0x0000000000000008,
      /* 8  */ 0x0000000000000008, 0x0000000000000008, 0x000000000A101017, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x0000000000000006, 0x000000000A191037, 0x0000000000000008,
      /* 9  */ 0x0000000000000008, 0x0000000000000008, "0x0A190A190A103083", 0x000000000A101079, 0x000019000A0A2347, 0x000000000A191057, 0x000000000A191057, 0x000000000A191057, 0x000000000A191057, 0x000000000A191057, 0x000000000A191057, 0x00000A190A192087, 0x0000000000000008,
      /* 10 */ 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008,
      /* 12 */ 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008, 0x0000000000000008,
      /* 13 */ 0x0000000000000008, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000, 0x0000000000000000,
    ].map(val=>BigInt(val)));
    this.size = Math.sqrt(this.wallGrid.length);

    makeAutoObservable(this);
  }

  hash(x: number, y: number) {
    return ((x * 73856093) ^ (y * 19349663)) % 100;
  }

  public stepTriangle = (
    rise: number,
    run: number,
    x: number,
    y: number,
    inverted?: boolean
  ): Point => {
    if (run === 0) return { length2: Infinity, x: undefined, y: undefined };
    let dx = run > 0 ? Math.floor(x + 1) - x : Math.ceil(x - 1) - x;
    let dy = dx * (rise / run);
    let newX = inverted ? y + dy : x + dx;
    let newY = inverted ? x + dx : y + dy;
    return {
      x: newX,
      y: newY,
      flooredX: Math.floor(x),
      flooredY: Math.floor(y),
      length2: dx * dx + dy * dy, // for the pythagorean theorem, so we can get the distance we made (which is the hypothenuse)
    };
  };

  public update = (seconds: number) => {
    if (this.light > 0) this.light = Math.max(this.light - 10 * seconds, 0);
    // else if (Math.random() * 5 < seconds) this.light = 2;
  };

  getSpriteTextureArray(): Int32Array {
    return new Int32Array([
      SpriteType.LADY,
      this.getSpriteTexture(SpriteType.LADY).texture.height,
      this.getSpriteTexture(SpriteType.LADY).texture.width,
      this.getSpriteData(SpriteType.LADY).angles,
      SpriteType.BUSH1,
      this.getSpriteTexture(SpriteType.BUSH1).texture.height,
      this.getSpriteTexture(SpriteType.BUSH1).texture.width,
      this.getSpriteData(SpriteType.BUSH1).angles,
      SpriteType.TREE_CONE,
      this.getSpriteTexture(SpriteType.TREE_CONE).texture.height,
      this.getSpriteTexture(SpriteType.TREE_CONE).texture.width,
      this.getSpriteData(SpriteType.TREE_CONE).angles,
      SpriteType.TREE_COLUMNAR,
      this.getSpriteTexture(SpriteType.TREE_COLUMNAR).texture.height,
      this.getSpriteTexture(SpriteType.TREE_COLUMNAR).texture.width,
      this.getSpriteData(SpriteType.TREE_COLUMNAR).angles,
      SpriteType.PILLAR,
      this.getSpriteTexture(SpriteType.PILLAR).texture.height,
      this.getSpriteTexture(SpriteType.PILLAR).texture.width,
      this.getSpriteData(SpriteType.PILLAR).angles,
      SpriteType.TREE_VASE,
      this.getSpriteTexture(SpriteType.TREE_VASE).texture.height,
      this.getSpriteTexture(SpriteType.TREE_VASE).texture.width,
      this.getSpriteData(SpriteType.TREE_VASE).angles,
      SpriteType.COLUMN,
      this.getSpriteTexture(SpriteType.COLUMN).texture.height,
      this.getSpriteTexture(SpriteType.COLUMN).texture.width,
      this.getSpriteData(SpriteType.COLUMN).angles,
    ]);
  }

  // TODO: map in rust?
  mapAngleToValue = (angle: number) => {
    let index = Math.round(angle / 45); // Default to 1 if the result is 0
    if (index === 8) {
      index = 0;
    }
    // Return the index of the closest midpoint
    return index;
  };

  public getSpriteTexture = (
    spriteType: SpriteType,
    angleVal: number = 0
  ): { texture: Bitmap } => {
    let texture: Bitmap;

    switch (spriteType) {
      case SpriteType.LADY:
        texture = this.ladyTextures[angleVal];
        break;
      case SpriteType.TREE_CONE:
        texture = this.treeTexture;
        break;
      case SpriteType.TREE_VASE:
        texture = this.treeTextureVase;
        break;
      case SpriteType.TREE_COLUMNAR:
        texture = this.treeTextureColumnar;
        break;
      case SpriteType.PILLAR:
        texture = this.pillarTexture;
        break;
      case SpriteType.BUSH1:
        texture = this.bush1Texture;
        break;
      case SpriteType.COLUMN:
        texture = this.windowTexture;
        break;
      default:
        console.log("Sprite texture not found for type " + spriteType);
        texture = this.treeTexture;
        break;
    }

    return { texture };
  };

  public getSpriteData = (spriteType: SpriteType): { angles: number } => {
    let angles = 1;

    switch (spriteType) {
      case SpriteType.LADY:
        angles = 8;
        break;
      case SpriteType.TREE_CONE:
        break;
      case SpriteType.TREE_VASE:
        break;
      case SpriteType.TREE_COLUMNAR:
        break;
      case SpriteType.PILLAR:
        break;
      case SpriteType.BUSH1:
        break;
      case SpriteType.COLUMN:
        break;
      default:
        console.log("Sprite texture not found for type " + spriteType);
        break;
    }

    return { angles };
  };
}
