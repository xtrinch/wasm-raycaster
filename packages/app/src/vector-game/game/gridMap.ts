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
import lady1Texture from "../../assets/woman/woman1.png";
import lady2Texture from "../../assets/woman/woman2.png";
import lady3Texture from "../../assets/woman/woman3.png";
import lady4Texture from "../../assets/woman/woman4.png";
import lady5Texture from "../../assets/woman/woman5.png";
import lady6Texture from "../../assets/woman/woman6.png";
import lady7Texture from "../../assets/woman/woman7.png";
import lady8Texture from "../../assets/woman/woman8.png";

import { Bitmap } from "./bitmap";
import { perlinNoise } from "./constants";
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
  public wallGrid: Uint32Array;
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
  public ladyTextures: Bitmap[];
  public light: number;

  constructor(size: number) {
    this.size = size;
    this.wallGrid = new Uint32Array(size * size);
    this.skybox = new Bitmap(panorama, 2000, 750);
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

    /*
     - 1: thick wall
     - 2: indoor floor / ceiling
     - 3: gravel road
     - 4: west door
     - 5: east door
     - 6: north door
     - 7: south door
     - 8: west door with floor
     - 9: east door with floor
     - 10: north door with floor
     - 11: south door with floor
     - 12: west door with gravel
     - 13: east door with gravel
     - 14: north door with gravel
     - 15: south door with gravel
     - 16: east-west door moved by N to the center
     - 17: north-south door moved by N to the center

     - 1x: east-west wall moved by N%10 to the west
     - 2x: east-west door moved by N%10 to the west
     - 3x: east-west door moved by N%10 to the west with floor
     - 4x: east-west door moved by N%10 to the west with gravel
     - 5x: north-south wall moved by N%10 to the south
     - 6x: north-south door moved by N%10 to the south
     - 7x: north-south door moved by N%10 to the south with floor
     - 8x: north-south door moved by N%10 to the south with gravel
    */
    // prettier-ignore
    //   this.wallGrid = new Uint8Array([
    //     /*       0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31 */
    //     /* 0  */ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 1  */ 3, 1, 1, 17,1, 1 ,1, 1, 1, 1, 1, 3, 0, 0, 3, 1, 1, 1, 1, 1, 2, 1, 1, 3, 0, 0, 1, 1, 1, 1, 1, 1,
    //     /* 2  */ 3, 30, 2, 2, 2, 2, 2, 2, 2, 2, 1, 3, 0, 0, 3, 1, 2, 2, 2, 2, 2, 2, 1, 3, 0, 0, 1, 0, 0, 0, 0, 1,
    //     /* 3  */ 3, 1, 2, 2, 2, 39, 2, 1, 1, 1, 1, 3, 0, 0, 3, 1, 2, 2, 2, 2, 2, 2, 1, 3, 0, 0, 1, 0, 0, 1, 0, 1,
    //     /* 4  */ 3, 76,2, 2, 2, 78, 2, 16, 3, 3, 3, 3, 0, 0, 3, 1, 1, 1, 2, 2, 2, 2, 1, 3, 0, 0, 1, 1, 1, 1, 0, 1,
    //     /* 5  */ 3, 1, 2, 2, 2, 2, 2, 1, 3, 3, 3, 3, 0, 0, 3, 3, 3, 1, 2, 2, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 6  */ 3, 1, 2, 2, 2, 37, 2, 1, 1, 1, 1, 3, 0, 0, 0, 0, 3, 1, 2, 2, 1, 3, 3, 3, 0, 0, 1, 1, 1, 1, 0, 1,
    //     /* 7  */ 3, 1, 2, 2, 2, 2, 2, 2, 2, 2, 9, 3, 3, 3, 3, 3, 3, 2, 2, 2, 1, 3, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1,
    //     /* 8  */ 3, 1, 1, 17,1, 1, 1, 1, 1, 1, 1, 3, 0, 0, 0, 0, 3, 1, 1, 1, 1, 3, 0, 0, 0, 0, 1, 1, 1, 1, 0, 1,
    //     /* 9  */ 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 10 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 11 */ 0, 0, 0, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    //     /* 12 */ 0, 0, 0, 3, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 13 */ 3, 3, 3, 3, 1, 2, 2, 1, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 14 */ 3, 1, 2, 1, 1, 2, 2, 1, 1, 1, 1, 3, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 15 */ 3, 1, 2, 2, 1, 2, 2, 1, 2, 2, 1, 3, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    //     /* 16 */ 3, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 17 */ 3, 1, 1, 1, 1, 2, 2, 1, 1, 1, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0,
    //     /* 18 */ 3, 3, 3, 3, 1, 2, 2, 1, 3, 3, 3, 3, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0,
    //     /* 19 */ 0, 0, 0, 3, 1, 2, 2, 1, 3, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 1, 0, 1, 0,
    //     /* 20 */ 0, 0, 0, 3, 1, 2, 2, 1, 3, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 21 */ 0, 0, 0, 3, 1, 2, 2, 1, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 22 */ 0, 0, 0, 3, 1, 2, 2, 1, 1, 1, 1, 3, 0, 1, 0, 0, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 23 */ 0, 0, 0, 3, 1, 2, 2, 2, 2, 2, 1, 3, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    //     /* 24 */ 0, 0, 0, 3, 1, 1, 2, 1, 1, 1, 1, 3, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 25 */ 0, 0, 0, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 26 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 27 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    //     /* 28 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 29 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    //     /* 30 */ 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 0, 0, 1, 1, 1, 0, 0, 0, 1, 1, 1, 1, 0, 1, 0,
    //     /* 31 */ 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0,
    // ]);

    // 32 bits, 4 bytes
    //     offset_secondary  width     thickness offset_primary    unset  north  door  thin_wall   road   ceiling floor  wall
    // BIN 0000              0000      0000      0000              0       0      0     0           0      0       0      0

    // - offset primary would be from east / north, and secondary for east would be north, and for north it will be east
    // 0x1 - wall (bit 0)
    // 0x2 - floor (bit 1)
    // 0x4 - ceiling (bit 2)
    // 0x6 - ceiling / floor (bit 1, 2)
    // 0x8 - road (bit 3)
    // 0x11 - thin wall (bit 0, 4)
    // 0x51 - thin wall - north (bit 0, 4, 6)
    // 0x17 - thin wall with ceiling / floor (bit 0, 1, 2, 4)
    // 0x19 - thin wall with road (bit 0, 3, 4)
    // 0x31 - thin door - east (bit 0, 4, 5)
    // 0x37 - thin door - east with wall & floor (bit 0, 1, 2, 4, 5)
    // 0x71 - thin door - north (bit 0, 4, 5, 6)

    this.wallGrid = new Uint32Array([
    /*       0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10,11 */
    /* 0  */ 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008,
    /* 1  */ 0x000008, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000008,
    /* 2  */ 0x000008, 0x000011, 0x000006, 0x000006, 0x281917, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x000001, 0x000008,
    /* 3  */ 0x000008, 0x000001, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x000001, 0x000001, 0x000001, 0x000001, 0x000008,
    /* 4  */ 0x000008, 0x000037, 0x000006, 0x223757, 0x281957, 0x000006, 0x000006, 0x000001, 0x000008, 0x000008, 0x000008, 0x000008,
    /* 5  */ 0x000008, 0x000001, 0x000006, 0x000006, 0x281917, 0x000006, 0x281957, 0x000001, 0x000008, 0x000008, 0x281959, 0x000008,
    /* 6  */ 0x000008, 0x000001, 0x000006, 0x000006, 0x281917, 0x281977, 0x281017, 0x000001, 0x000001, 0x000001, 0x281917, 0x000008,
    /* 7  */ 0x000008, 0x000001, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x000006, 0x281937, 0x000008,
    /* 8  */ 0x000008, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x000001, 0x281917, 0x000008,
    /* 10 */ 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x281059, 0x000008,
    /* 10 */ 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008, 0x000008,
    /* 11 */ 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000, 0x000000,

  ]);
    makeAutoObservable(this);
  }

  // returns 1 or 0, depending on whether there is a wall at that point
  public get = (x: number, y: number): number => {
    x = Math.floor(x);
    y = Math.floor(y);
    if (x < 0 || x > this.size - 1 || y < 0 || y > this.size - 1) return -1;
    return this.wallGrid[y * this.size + x];
  };

  // old function which randomizes the world
  public randomize = (): void => {
    for (let i = 0; i < this.size * this.size; i++) {
      this.wallGrid[i] = Math.random() < 0.3 ? 1 : 0;
    }
  };

  hash(x: number, y: number) {
    return ((x * 73856093) ^ (y * 19349663)) % 100;
  }
  generateWorld(): void {
    // Step 1: Generate walls using noise
    for (let y = 0; y < this.size; y++) {
      for (let x = 0; x < this.size; x++) {
        // let elevation = noise2D(x * 0.1, y * 0.1);
        let elevation = perlinNoise.perlin2(x * 0.1, y * 0.1);
        elevation = (elevation * 5) << 2;
        this.wallGrid[y * this.size + x] =
          elevation > 1 && elevation < 9 ? 1 : 0; // 1 = wall, 0 = empty space
      }
    }

    return;
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

  public inspectForWall = (
    sin: number,
    cos: number,
    nextStep: Point,
    shiftX: number,
    shiftY: number,
    distance: number,
    offset: number,
    hitWall: boolean
  ): Point => {
    let dx = cos < 0 ? shiftX : 0;
    let dy = sin < 0 ? shiftY : 0;
    nextStep.height = 0;

    // skip checking if we're already hit a wall
    if (!hitWall) {
      // height=1 if there is a wall
      const gridItem = this.get(nextStep.x - dx, nextStep.y - dy);
      if (gridItem === 2) {
        nextStep.height = 0.75;
        nextStep.type = "tree";
      } else if (gridItem === 1) {
        nextStep.height = 1;
        nextStep.type = "wall";
      }
    }
    nextStep.distance = distance + Math.sqrt(nextStep.length2);

    if (shiftX) nextStep.shading = cos < 0 ? 2 : 0;
    else nextStep.shading = sin < 0 ? 2 : 1;

    nextStep.offset = offset - Math.floor(offset);
    return nextStep;
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
      SpriteType.BUSH1,
      this.getSpriteTexture(SpriteType.BUSH1).texture.height,
      this.getSpriteTexture(SpriteType.BUSH1).texture.width,
      SpriteType.TREE_CONE,
      this.getSpriteTexture(SpriteType.TREE_CONE).texture.height,
      this.getSpriteTexture(SpriteType.TREE_CONE).texture.width,
      SpriteType.TREE_COLUMNAR,
      this.getSpriteTexture(SpriteType.TREE_COLUMNAR).texture.height,
      this.getSpriteTexture(SpriteType.TREE_COLUMNAR).texture.width,
      SpriteType.PILLAR,
      this.getSpriteTexture(SpriteType.PILLAR).texture.height,
      this.getSpriteTexture(SpriteType.PILLAR).texture.width,
      SpriteType.TREE_VASE,
      this.getSpriteTexture(SpriteType.TREE_VASE).texture.height,
      this.getSpriteTexture(SpriteType.TREE_VASE).texture.width,
    ]);
  }

  // TODO: map in rust?
  mapAngleToValue = (angle) => {
    let index = Math.round(angle / 45); // Default to 1 if the result is 0
    if (index === 8) {
      index = 0;
    }
    // Return the index of the closest midpoint
    return index;
  };

  public getSpriteTexture = (spriteType: SpriteType, angle: number = 0) => {
    const angleVal = this.mapAngleToValue(angle);
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
    }

    return { texture };
  };
}
