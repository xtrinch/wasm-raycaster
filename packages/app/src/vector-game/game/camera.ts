import { flatten } from "lodash";
import { makeAutoObservable } from "mobx";
import {
  draw_ceiling_floor_raycast,
  draw_sprites_wasm,
  draw_walls_raycast,
  WasmFloat32Array,
  WasmInt32Array,
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
  public widthResolution: number; // how many columns we draw
  public heightResolution: number; // how many scanlines we draw
  public ceilingWidthResolution: number; // how many columns we draw
  public ceilingHeightResolution: number; // how many scanlines we draw
  public widthSpacing: number;
  public heightSpacing: number;
  public ceilingWidthSpacing: number;
  public ceilingHeightSpacing: number;
  public range: number;
  public lightRange: number;
  public scale: number;
  public skipCounter: number;
  public initialSkipCounter: number;
  public context: CanvasRenderingContext2D;
  public canvas: HTMLCanvasElement;
  public map: GridMap;
  public originalCanvas: HTMLCanvasElement;
  public ceilingFloorPixelsPtr: number;
  public ceilingFloorPixelsRef: WasmUint8Array;
  public columnsRef: WasmInt32Array;
  public floorTextureRef: WasmUint8Array;
  public ceilingTextureRef: WasmUint8Array;
  public roadTextureRef: WasmUint8Array;
  public doorTextureRef: WasmUint8Array;
  public visibleSpritesRef: WasmFloat32Array;
  public spritePartsRef: WasmInt32Array;
  public allSpritesRef: WasmFloat32Array;
  public zBufferRef: WasmFloat32Array;
  public spritesTextureRef: WasmInt32Array;
  public mapRef: WasmUInt64Array;
  public initialized: boolean;

  constructor(canvas: HTMLCanvasElement, map: GridMap, spriteMap: SpriteMap) {
    this.ctx = canvas.getContext("2d");
    this.width = canvas.width = window.innerWidth;
    this.width = this.width;
    this.height = canvas.height = window.innerHeight;
    this.height = this.height;

    // note that this should be whole numbers
    this.widthSpacing = 1;
    this.heightSpacing = 1;
    this.ceilingWidthSpacing = 2;
    this.ceilingHeightSpacing = 2;

    this.widthResolution = Math.ceil(this.width / this.widthSpacing);
    this.heightResolution = Math.ceil(this.height / this.heightSpacing);
    this.ceilingWidthResolution = Math.ceil(
      this.width / this.ceilingWidthSpacing
    );
    this.ceilingHeightResolution = Math.ceil(
      this.height / this.ceilingHeightSpacing
    );

    this.range = 40;
    this.lightRange = 15;
    this.scale = (this.width + this.height) / 1200;
    this.initialSkipCounter = 1;
    this.skipCounter = this.initialSkipCounter;
    this.map = map;
    this.originalCanvas = canvas;

    this.initializeTexture(this.map.floorTexture, "floorTextureRef");
    this.initializeTexture(this.map.ceilingTexture, "ceilingTextureRef");
    this.initializeTexture(this.map.roadTexture, "roadTextureRef");
    this.initializeTexture(this.map.doorTexture, "doorTextureRef");

    let length = this.ceilingWidthResolution * this.ceilingHeightResolution * 4;

    // ensure we're passing the data in all the same memory locations
    this.ceilingFloorPixelsRef = new WasmUint8Array(length);
    this.columnsRef = new WasmInt32Array(this.widthResolution * 8 * 8);
    this.allSpritesRef = new WasmFloat32Array(spriteMap.size * 5); // this will be the max sprites there will ever be in here
    this.allSpritesRef.set(
      new Float32Array(
        flatten(
          spriteMap.sprites.map((s) => [s[0], s[1], s[2], s[3] * 100, s[4]])
        )
      )
    );
    this.spritePartsRef = new WasmInt32Array(
      (spriteMap.size + // this will be the max sprites there will ever be in here
        2 * this.widthResolution) * // two times the columns to account for windows
        5 *
        2 // we'll expect at most two parts for each
    ); // this will be the max sprites there will ever be in here

    // TODO: don't think this is necessary now that we don't pass it around
    this.visibleSpritesRef = new WasmFloat32Array(
      (spriteMap.size + // this will be the max sprites there will ever be in here
        2 * this.widthResolution) * // two times the columns to account for windows
        9
    );
    this.zBufferRef = new WasmFloat32Array(this.widthResolution);

    this.spritesTextureRef = new WasmInt32Array(
      Object.values(SpriteType).length * 3
    );
    this.spritesTextureRef.set(map.getSpriteTextureArray());
    this.mapRef = new WasmUInt64Array(map.size * map.size);
    this.mapRef.set(map.wallGrid);

    makeAutoObservable(this);
  }

  async initializeTexture(texture: Bitmap, refKey: string) {
    const img = texture.image;
    const canvas = document.createElement("canvas") as HTMLCanvasElement;
    this.context = canvas.getContext("2d");
    canvas.width = texture.width * 2;
    canvas.height = texture.height * 2;
    texture.image.onload = () => {
      this.context.drawImage(img, 0, 0, texture.width, texture.height);
      const data = this.context.getImageData(
        0,
        0,
        texture.width,
        texture.height
      )?.data;
      this[refKey] = new WasmUint8Array(texture.width * texture.height * 4);
      (this[refKey] as WasmUint8Array).set(data as any as Uint8Array);
    };
  }

  render(player: Player, map: GridMap, spriteMap: SpriteMap) {
    if (
      !this.ceilingTextureRef ||
      !this.floorTextureRef ||
      !this.roadTextureRef ||
      !this.doorTextureRef
    ) {
      return;
    }

    this.ctx.save();
    this.ctx.fillStyle = "#000000";
    this.ctx.fillRect(0, 0, this.width, this.height);
    this.ctx.restore();
    this.drawSky(player, map.skybox, map.light);
    this.drawColumns(player, map, spriteMap);
    this.drawWeapon(player.weapon, player.paces);
  }

  drawSky(player: Player, sky: Bitmap, ambient: number) {
    const direction =
      Math.atan2(player.position.dirX, player.position.dirY) + Math.PI;
    const y = player.position.pitch;

    let width = sky.width * (this.height / sky.height) * 2;
    let CIRCLE = Math.PI * 2;
    let left = (direction / CIRCLE) * -width;

    this.ctx.save();
    this.ctx.drawImage(sky.image, left, y, width, this.height);
    if (left < width - this.width) {
      this.ctx.drawImage(sky.image, left + width, y, width, this.height);
    }
    if (ambient > 0) {
      this.ctx.fillStyle = "#ffffff";
      this.ctx.globalAlpha = ambient * 0.1;
      this.ctx.fillRect(0, this.height * 0.5, this.width, this.height * 0.5);
    }
    this.ctx.restore();
  }

  scaleCanvasImage(
    buf: Uint8Array,
    width: number,
    height: number
  ): HTMLCanvasElement {
    // Create a temporary canvas
    const tempCanvas = document.createElement("canvas");
    const tempCtx = tempCanvas.getContext("2d");

    // Set the canvas size to match the image data
    tempCanvas.width = width;
    tempCanvas.height = height;

    // Create an ImageData object
    const img01 = new ImageData(
      new Uint8ClampedArray(buf),
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    // Draw ImageData onto the temporary canvas
    tempCtx.putImageData(img01, 0, 0);

    return tempCanvas;
  }

  drawCeilingFloorRaycastWasm(player: Player, map: GridMap) {
    draw_ceiling_floor_raycast(
      player.toRustPosition(),
      this.ceilingFloorPixelsRef.ptr,
      this.floorTextureRef.ptr,
      this.ceilingTextureRef.ptr,
      this.roadTextureRef.ptr,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution,
      this.ceilingWidthSpacing,
      this.ceilingHeightSpacing,
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
      map.size // Width of original 2D array
    );

    const tempCanvas1 = this.scaleCanvasImage(
      this.ceilingFloorPixelsRef.buffer,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );
    this.ctx.drawImage(tempCanvas1, 0, 0, this.width, this.height);
  }

  drawWallsRaycastWasm(
    player: Player,
    map: GridMap,
    spriteMap: SpriteMap
  ): number {
    let foundSpritesCount = draw_walls_raycast(
      this.columnsRef.ptr,
      this.zBufferRef.ptr,
      player.toRustPosition(),
      this.mapRef.ptr,
      map.size, // Width of original 2D array
      this.widthResolution,
      this.height,
      this.width,
      this.widthSpacing,
      this.lightRange,
      this.range,
      map.wallTexture.width,
      this.visibleSpritesRef.ptr,
      this.allSpritesRef.ptr,
      spriteMap.size
    );
    let width = Math.ceil(this.widthSpacing);
    for (let idx = 0; idx < this.columnsRef.buffer.length / 8; idx += 8) {
      let [
        tex_x,
        left,
        draw_start_y,
        wall_height,
        global_alpha,
        hit,
        hit_type,
      ] = this.columnsRef.buffer.slice(idx, idx + 7);

      if (hit) {
        let texture: Bitmap;
        switch (hit_type) {
          case 1:
            texture = map.wallTexture;
            break;
          case 2:
            texture = map.doorTexture;
            break;
          case 3:
            texture = map.windowTexture;
            break;
        }
        this.ctx.drawImage(
          texture.image,
          tex_x, // sx
          0, // sy
          1, // sw
          texture.height, // sh
          left, // dx
          draw_start_y, // dy - yes we go into minus here, it'll be ignored anyway
          width, // dw
          wall_height // dh
        );

        this.ctx.save();
        this.ctx.globalAlpha = global_alpha / 100;

        // black overlay to simulate darkness
        this.ctx.fillRect(left, draw_start_y, width, wall_height);
        this.ctx.restore();
        // this.ctx.globalAlpha = 1;
      }
    }

    return foundSpritesCount;
  }

  drawSpritesWasm(
    player: Player,
    map: GridMap,
    foundSpritesCount: number
  ): void {
    const stripePartCount = draw_sprites_wasm(
      player.toRustPosition(),
      this.width,
      this.height,
      this.widthSpacing,
      this.visibleSpritesRef.ptr,
      this.spritePartsRef.ptr,
      this.zBufferRef.ptr,
      this.spritesTextureRef.ptr,
      Object.values(SpriteType).length * 3,
      this.lightRange,
      map.light,
      this.widthResolution,
      this.heightResolution,
      foundSpritesCount
    );
    for (let stripeIdx = 0; stripeIdx < stripePartCount; stripeIdx++) {
      const arrayIdx = stripeIdx * 9;
      const [
        spriteType,
        stripeLeftX,
        width,
        screenYCeiling,
        height,
        texX1,
        texX2,
        alpha,
        angle,
      ] = this.spritePartsRef.buffer.slice(arrayIdx, arrayIdx + 9);
      const { texture } = map.getSpriteTexture(spriteType, angle);

      this.ctx.save();
      // TODO: this is slow, fix
      if (spriteType !== SpriteType.COLUMN) {
        this.ctx.filter = `brightness(${alpha}%)`; // min 20% brightness
        // this can be used for sprites but not for windows (there we should use a black overlay)
      }
      this.ctx.drawImage(
        texture.image,
        texX1, // sx
        0, // sy
        texX2 - texX1, // sw
        texture.height, // sh
        stripeLeftX, // dx
        screenYCeiling, // dy
        width, // dw
        height // dh
      );

      if (spriteType === SpriteType.COLUMN) {
        this.ctx.globalAlpha = 1 - alpha / 100;

        // black overlay to simulate darkness
        this.ctx.fillRect(stripeLeftX, screenYCeiling, width, height);
      }
      this.ctx.restore();
    }
  }

  // draws columns left to right
  drawColumns(player: Player, map: GridMap, spriteMap: SpriteMap) {
    this.ctx.save();

    this.drawCeilingFloorRaycastWasm(player, map);
    const foundSpritesCount = this.drawWallsRaycastWasm(player, map, spriteMap);
    this.drawSpritesWasm(player, map, foundSpritesCount);

    this.ctx.restore();
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
