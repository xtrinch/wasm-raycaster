import { isNumber, range } from "lodash";
import { makeAutoObservable } from "mobx";
import {
  BackgroundImageWasm,
  render,
  TextureType,
  WasmFloat32Array,
  WasmTextureMap,
  WasmTextureMetaMap,
  WasmUint8Array,
} from "../../../wasm";
import { Bitmap } from "./bitmap";
import { GridMap } from "./gridMap";
import { Player } from "./player";

export interface Sprite {
  x: number;
  y: number;
  angle: number;
  type: TextureType;
}

export class Camera {
  public ctx: CanvasRenderingContext2D;
  public width: number;
  public height: number;
  public range: number;
  public lightRange: number;
  public scale: number;
  public canvas: HTMLCanvasElement;
  public map: GridMap;
  public pixelsRef: WasmUint8Array;
  // Backed by a plain ArrayBuffer: ImageData cannot be built on shared WASM memory.
  public pixelsClampedArray: Uint8ClampedArray<ArrayBuffer>;
  public skyTextureRef: WasmUint8Array;
  // Wall depth per screen column.
  public columnDepthRef: WasmFloat32Array;
  // Nearest floor, ceiling or roof depth per pixel.
  public depthRef: WasmFloat32Array; // nearest floor, ceiling, roof or model depth per pixel
  public initialized: boolean;
  public spriteTextureHashMap: WasmTextureMap;
  public backgroundRef: BackgroundImageWasm;
  public spriteTextureMetaHashMap: WasmTextureMetaMap;

  constructor(canvas: HTMLCanvasElement, map: GridMap) {
    this.ctx = canvas.getContext("2d", { alpha: false });
    this.width = canvas.width = (8 * window.innerWidth) / 8;
    this.height = canvas.height = (8 * window.innerHeight) / 8;
    this.width = this.width + 4 - (this.width % 4);
    this.height = this.height + 4 - (this.height % 4);

    this.range = 40;
    this.lightRange = 15;
    this.scale = (this.width + this.height) / 1200;
    this.map = map;

    this.backgroundRef = new BackgroundImageWasm();

    this.initializeTexture(this.map.skybox, "skyTextureRef", () => {
      this.backgroundRef.set(
        this.skyTextureRef.ptr,
        this.map.skybox.width,
        this.map.skybox.height,
        this.width,
        this.height
      );
    });

    let length = this.width * this.height * 4;

    // ensure we're passing the data in all the same memory locations
    this.pixelsRef = new WasmUint8Array(length);
    this.pixelsClampedArray = new Uint8ClampedArray(new ArrayBuffer(length));

    this.columnDepthRef = new WasmFloat32Array(this.width);
    this.depthRef = new WasmFloat32Array(this.width * this.height);

    this.spriteTextureHashMap = new WasmTextureMap();
    this.populateSpriteTextureHashMap();

    this.spriteTextureMetaHashMap = new WasmTextureMetaMap();
    for (let textureType of Object.values(TextureType).filter(
      isNumber as any
    ) as number[]) {
      const height = this.map.getSpriteTexture(textureType).texture.height;
      const width = this.map.getSpriteTexture(textureType).texture.width;
      const angles = this.map.getSpriteData(textureType).angles;
      this.spriteTextureMetaHashMap.populateFromArray(
        textureType,
        width,
        height,
        angles
      );
    }

    makeAutoObservable(this);
  }

  populateSpriteTextureHashMap() {
    Object.values(TextureType)
      .filter(isNumber as any)
      .map((val: number) => {
        let { angles } = this.map.getSpriteData(val);
        for (let angle of range(0, angles)) {
          let texture = this.map.getSpriteTexture(val, angle).texture;
          this.initializeSpriteTexture(texture, val, angle);
        }
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

  async initializeSpriteTexture(
    texture: Bitmap,
    refKey: number,
    angle: number
  ) {
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
        angle,
        data as any as Uint8Array
      );
    };
  }

  render(player: Player) {
    if (
      !this.skyTextureRef ||
      !this.backgroundRef ||
      this.spriteTextureHashMap.count_cells() < 20 // Wait for 19 existing texture entries plus the roof.
    ) {
      return;
    }

    render(
      player.position.x,
      player.position.y,
      player.position.dir_x,
      player.position.dir_y,
      player.position.plane_x,
      player.position.plane_y,
      player.position.pitch,
      player.position.z,
      player.position.plane_y_initial,
      this.pixelsRef.ptr,
      this.columnDepthRef.ptr,
      this.map.world,
      this.width,
      this.height,
      this.lightRange,
      this.range,
      this.map.light,
      this.backgroundRef,
      this.spriteTextureHashMap,
      this.spriteTextureMetaHashMap,
      this.depthRef.ptr
    );

    this.drawWeapon(player.weapon, player.paces);
  }

  drawCanvas() {
    // this.imageData.data.set(this.pixelsRef.buffer);

    this.pixelsClampedArray.set(this.pixelsRef.buffer);
    const img01 = new ImageData(
      this.pixelsClampedArray,
      this.width,
      this.height
    );

    this.ctx.putImageData(img01, 0, 0);
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
