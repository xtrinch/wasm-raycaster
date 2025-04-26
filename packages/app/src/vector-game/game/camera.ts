import { flatten, isNumber } from "lodash";
import { makeAutoObservable } from "mobx";
import {
  BackgroundImageWasm,
  draw_background_image_prescaled,
  draw_ceiling_floor_raycast,
  draw_sprites_wasm,
  draw_walls_raycast,
  WasmFloat32Array,
  WasmInt32Array,
  WasmStripeHashMapArray,
  WasmStripeTextureHashMapArray,
  WasmUInt64Array,
  WasmUint8Array,
} from "../../../wasm";
import { Bitmap } from "./bitmap";
import { GridMap } from "./gridMap";
import { Player } from "./player";
import { SpriteMap, SpriteType } from "./spriteMap";

export interface Sprite {
  x: number;
  y: number;
  angle: number;
  type: SpriteType;
}

export class Camera {
  public ctx: CanvasRenderingContext2D;
  public width: number;
  public height: number;
  public range: number;
  public lightRange: number;
  public scale: number;
  public skipCounter: number;
  public initialSkipCounter: number;
  public context: CanvasRenderingContext2D;
  public canvas: HTMLCanvasElement;
  public map: GridMap;
  public originalCanvas: HTMLCanvasElement;
  public ceilingFloorPixelsRef: WasmUint8Array;
  public pixelsClampedArray: Uint8ClampedArray;
  public columnsRef: WasmInt32Array;
  public floorTextureRef: WasmUint8Array;
  public ceilingTextureRef: WasmUint8Array;
  public skyTextureRef: WasmUint8Array;
  public roadTextureRef: WasmUint8Array;
  public doorTextureRef: WasmUint8Array;
  public treeTextureRef: WasmUint8Array;
  public windowTextureRef: WasmUint8Array;
  public wallTextureRef: WasmUint8Array;
  public visibleSpritesRef: WasmFloat32Array;
  public spritePartsRef: WasmInt32Array;
  public allSpritesRef: WasmFloat32Array;
  public zBufferRef: WasmFloat32Array;
  public spritesTextureRef: WasmInt32Array;
  public mapRef: WasmUInt64Array;
  public initialized: boolean;
  public spriteHashMap: WasmStripeHashMapArray; // sprites per coordinate
  public spriteTextureHashMap: WasmStripeTextureHashMapArray;
  public backgroundRef: BackgroundImageWasm;

  constructor(canvas: HTMLCanvasElement, map: GridMap, spriteMap: SpriteMap) {
    this.ctx = canvas.getContext("2d", { alpha: false });
    this.width = canvas.width = (8 * window.innerWidth) / 8;
    this.height = canvas.height = (8 * window.innerHeight) / 8;
    this.width = this.width + 4 - (this.width % 4);
    this.height = this.height + 4 - (this.height % 4);

    this.range = 40;
    this.lightRange = 15;
    this.scale = (this.width + this.height) / 1200;
    this.initialSkipCounter = 1;
    this.skipCounter = this.initialSkipCounter;
    this.map = map;
    this.originalCanvas = canvas;

    this.initializeTexture(this.map.windowTexture, "windowTextureRef");
    this.initializeTexture(this.map.floorTexture, "floorTextureRef");
    this.initializeTexture(this.map.ceilingTexture, "ceilingTextureRef");
    this.initializeTexture(this.map.roadTexture, "roadTextureRef");
    this.initializeTexture(this.map.doorTexture, "doorTextureRef");
    this.initializeTexture(this.map.wallTexture, "wallTextureRef");
    // pre-scale background image to fit the browser window
    this.initializeTexture(this.map.skybox, "skyTextureRef", () => {
      this.backgroundRef = new BackgroundImageWasm(
        this.skyTextureRef.ptr,
        this.map.skybox.width,
        this.map.skybox.height,
        this.width,
        this.height
      );
    });
    this.initializeTexture(this.map.treeTexture, "treeTextureRef");

    let length = this.width * this.height * 4;

    // ensure we're passing the data in all the same memory locations
    this.ceilingFloorPixelsRef = new WasmUint8Array(length);
    this.pixelsClampedArray = new Uint8ClampedArray(length);
    this.columnsRef = new WasmInt32Array(this.width * 8 * 8);
    this.allSpritesRef = new WasmFloat32Array(spriteMap.size * 5); // this will be the max sprites there will ever be in here
    const allSprites = new Float32Array(
      flatten(
        spriteMap.sprites.map((s) => [s[0], s[1], s[2], s[3] * 100, s[4]])
      )
    );
    this.allSpritesRef.set(allSprites);
    this.spritePartsRef = new WasmInt32Array(
      (spriteMap.size + // this will be the max sprites there will ever be in here
        2 * this.width) * // two times the columns to account for windows
        9
    ); // this will be the max sprites there will ever be in here

    // TODO: don't think this is necessary now that we don't pass it around
    this.visibleSpritesRef = new WasmFloat32Array(
      (spriteMap.size + // this will be the max sprites there will ever be in here
        2 * this.width) * // two times the columns to account for windows
        9
    );
    this.zBufferRef = new WasmFloat32Array(this.width);

    this.spritesTextureRef = new WasmInt32Array(
      Object.values(SpriteType).length * 3
    );
    this.spritesTextureRef.set(map.getSpriteTextureArray());
    this.mapRef = new WasmUInt64Array(map.size * map.size);
    this.mapRef.set(map.wallGrid);

    this.spriteHashMap = new WasmStripeHashMapArray();
    this.spriteHashMap.populateFromArray(allSprites); // No memory allocation needed!

    this.spriteTextureHashMap = new WasmStripeTextureHashMapArray();
    this.populateSpriteTextureHashMap();

    makeAutoObservable(this);
  }

  populateSpriteTextureHashMap() {
    Object.values(SpriteType)
      .filter(isNumber as any)
      .map((val: number) => {
        let texture = this.map.getSpriteTexture(val).texture;
        this.initializeSpriteTexture(texture, val);
      });
  }

  async initializeTexture(texture: Bitmap, refKey: string, func?: Function) {
    const img = texture.image;
    const canvas = document.createElement("canvas") as HTMLCanvasElement;
    const tmpContext = canvas.getContext("2d");
    canvas.width = texture.width * 2;
    canvas.height = texture.height * 2;
    texture.image.onload = () => {
      tmpContext.drawImage(img, 0, 0, texture.width, texture.height);
      const data = tmpContext.getImageData(
        0,
        0,
        texture.width,
        texture.height
      )?.data;
      this[refKey] = new WasmUint8Array(texture.width * texture.height * 4);
      (this[refKey] as WasmUint8Array).set(data as any as Uint8Array);
      if (func) {
        func();
      }
    };
  }

  async initializeSpriteTexture(texture: Bitmap, refKey: number) {
    const img = texture.image;
    const canvas = document.createElement("canvas") as HTMLCanvasElement;
    const tmpContext = canvas.getContext("2d");
    canvas.width = texture.width * 2;
    canvas.height = texture.height * 2;
    texture.image.onload = () => {
      tmpContext.drawImage(img, 0, 0, texture.width, texture.height);
      const data = tmpContext.getImageData(
        0,
        0,
        texture.width,
        texture.height
      )?.data;

      this.spriteTextureHashMap.populateFromArray(
        refKey,
        0,
        data as any as Uint8Array
      );
    };
  }

  render(player: Player, map: GridMap, spriteMap: SpriteMap) {
    if (
      !this.ceilingTextureRef ||
      !this.floorTextureRef ||
      !this.roadTextureRef ||
      !this.doorTextureRef ||
      !this.skyTextureRef ||
      !this.backgroundRef
    ) {
      return;
    }

    this.drawSky(player, map.light);
    this.drawCeilingFloorRaycastWasm(player, map);
    const foundSpritesCount = this.drawWallsRaycastWasm(player, map, spriteMap);
    this.drawSpritesWasm(player, map, foundSpritesCount);
    this.drawWeapon(player.weapon, player.paces);
  }

  drawSky(player: Player, ambient: number) {
    // draw_background_image(
    //   this.skyTextureRef.ptr,
    //   this.ceilingFloorPixelsRef.ptr,
    //   this.map.skybox.width,
    //   this.map.skybox.height,
    //   this.width,
    //   this.height,
    //   ambient,
    //   player.position.dir_x,
    //   player.position.dir_y,
    //   player.position.pitch
    // );
    draw_background_image_prescaled(
      this.backgroundRef,
      this.ceilingFloorPixelsRef.ptr,
      this.width,
      this.height,
      player.position.dir_x,
      player.position.dir_y,
      player.position.pitch
    );
  }

  drawCanvas() {
    this.pixelsClampedArray.set(this.ceilingFloorPixelsRef.buffer);
    const img01 = new ImageData(
      this.pixelsClampedArray,
      this.width,
      this.height
    );

    this.ctx.putImageData(img01, 0, 0);
  }

  drawCeilingFloorRaycastWasm(player: Player, map: GridMap) {
    draw_ceiling_floor_raycast(
      this.ceilingFloorPixelsRef.ptr,
      this.floorTextureRef.ptr,
      this.ceilingTextureRef.ptr,
      this.roadTextureRef.ptr,
      this.width,
      this.height,
      this.height,
      this.lightRange,
      map.light,
      map.floorTexture.width,
      map.floorTexture.height,
      map.ceilingTexture.width,
      map.ceilingTexture.height,
      map.roadTexture.width,
      map.roadTexture.height,
      this.mapRef.ptr, // 1D array instead of 2D
      map.size, // Width of original 2D array
      player.position.x,
      player.position.y,
      player.position.dir_x,
      player.position.dir_y,
      player.position.plane_x,
      player.position.plane_y,
      player.position.pitch,
      player.position.z,
      player.position.plane_y_initial
    );
  }

  drawWallsRaycastWasm(
    player: Player,
    map: GridMap,
    spriteMap: SpriteMap
  ): number {
    let foundSpritesCount = draw_walls_raycast(
      this.ceilingFloorPixelsRef.ptr,
      this.wallTextureRef.ptr,
      this.doorTextureRef.ptr,
      this.zBufferRef.ptr,
      this.mapRef.ptr,
      map.size, // Width of original 2D array
      this.width,
      this.height,
      this.lightRange,
      this.range,
      map.wallTexture.width,
      map.wallTexture.height,
      map.doorTexture.width,
      map.doorTexture.height,
      this.visibleSpritesRef.ptr,
      spriteMap.size,
      this.spriteHashMap,
      player.position.x,
      player.position.y,
      player.position.dir_x,
      player.position.dir_y,
      player.position.plane_x,
      player.position.plane_y,
      player.position.pitch,
      player.position.z,
      player.position.plane_y_initial
    );

    return foundSpritesCount;
  }

  drawSpritesWasm(
    player: Player,
    map: GridMap,
    foundSpritesCount: number
  ): void {
    draw_sprites_wasm(
      this.ceilingFloorPixelsRef.ptr,
      this.width,
      this.height,
      this.visibleSpritesRef.ptr,
      this.zBufferRef.ptr,
      this.spritesTextureRef.ptr,
      Object.values(SpriteType).length * 3,
      this.lightRange,
      map.light,
      foundSpritesCount,
      player.position.x,
      player.position.y,
      player.position.dir_x,
      player.position.dir_y,
      player.position.plane_x,
      player.position.plane_y,
      player.position.pitch,
      player.position.z,
      player.position.plane_y_initial,
      this.spriteTextureHashMap
    );
  }

  drawWeapon(weapon: Bitmap, paces: number): void {
    let bobX = Math.cos(paces * 2) * this.scale * 6;
    let bobY = Math.sin(paces * 4) * this.scale * 6;
    let left = this.width * 0.66 + bobX;
    let top = this.height * 0.6 + bobY;
    this.ctx.drawImage(
      weapon.image,
      left,
      top,
      weapon.width * this.scale,
      weapon.height * this.scale
    );
  }
}
