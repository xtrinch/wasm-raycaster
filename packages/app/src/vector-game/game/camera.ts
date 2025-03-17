import { flatten, sortBy } from "lodash";
import { makeAutoObservable } from "mobx";
import {
  draw_ceiling_floor_raycast,
  raycast_visible_coordinates,
  translate_coordinate_to_camera,
  WasmFloat32Array,
  WasmInt32Array,
  WasmUint8Array,
} from "../../../wasm";
import { draw_walls_raycast } from "../../wasm";
import { Bitmap } from "./bitmap";
import { GridMap } from "./gridMap";
import { Player } from "./player";
import { SpriteMap, SpriteType } from "./spriteMap";

interface CoordItem {
  x: number;
  y: number;
  distance: number;
  hasWall: boolean;
  hasCeilingFloor: boolean;
  visibleSquares: {
    [key: string]: number[];
  };
}
interface Coords {
  [key: string]: // key `${x}-${y}`
  CoordItem;
}

export interface Sprite {
  x: number;
  y: number;
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
  public ceilingFloorBlackPixelsRef: WasmUint8Array;
  public columnsRef: WasmInt32Array;
  public floorTextureRef: WasmUint8Array;
  public ceilingTextureRef: WasmUint8Array;
  public spritesRef: WasmFloat32Array;
  public zBufferRef: WasmFloat32Array;

  constructor(canvas: HTMLCanvasElement, map: GridMap, spriteMap: SpriteMap) {
    this.ctx = canvas.getContext("2d");
    this.width = canvas.width = window.innerWidth;
    this.height = canvas.height = window.innerHeight;
    this.widthResolution = this.width; //620;
    this.heightResolution = 420;
    const factor = 2 / 5;
    this.ceilingHeightResolution =
      this.width * factor - ((this.width * factor) % 2); //650;
    this.ceilingWidthResolution =
      this.height * factor - ((this.height * factor) % 2); //550;
    this.widthSpacing = this.width / this.widthResolution;
    this.heightSpacing = this.height / this.heightResolution;
    this.ceilingWidthSpacing = this.width / this.ceilingWidthResolution;
    this.ceilingHeightSpacing = this.height / this.ceilingHeightResolution;
    this.range = 40;
    this.lightRange = 15;
    this.scale = (this.width + this.height) / 1200;
    this.initialSkipCounter = 1;
    this.skipCounter = this.initialSkipCounter;
    this.map = map;
    this.originalCanvas = canvas;
    this.intializeTexture(this.map.floorTexture, "floorTextureRef");
    this.intializeTexture(this.map.ceilingTexture, "ceilingTextureRef");

    let length = this.ceilingWidthResolution * this.ceilingHeightResolution * 4;

    // ensure we're passing the data in all the same memory locations
    this.ceilingFloorPixelsRef = new WasmUint8Array(length);
    this.ceilingFloorBlackPixelsRef = new WasmUint8Array(length);
    this.columnsRef = new WasmInt32Array(this.widthResolution * 7 * 8);
    this.spritesRef = new WasmFloat32Array(spriteMap.size * 3);
    this.spritesRef.set(new Float32Array(flatten(spriteMap.sprites)));
    this.zBufferRef = new WasmFloat32Array(this.widthResolution);

    makeAutoObservable(this);
  }

  intializeTexture(texture: Bitmap, refKey: string) {
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
    const y = player.position.pitch + player.position.z;

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

  raycastVisibleCoordinatesWasm(
    spriteMap: SpriteMap,
    player: Player,
    map: GridMap
  ): { coords: Coords; sprites: Sprite[] } {
    const data: { coords: Map<any, any>; sprites: Sprite[] } =
      raycast_visible_coordinates(
        player.toRustPosition(),
        this.widthResolution,
        this.range,
        map.wallGrid, // 1D array instead of 2D
        map.size, // Width of original 2D array
        new Float32Array(flatten(spriteMap.sprites))
      );
    return { sprites: data.sprites, coords: Object.fromEntries(data.coords) };
  }

  async drawCeilingFloorRaycastWasm(player: Player, map: GridMap) {
    if (!this.ceilingTextureRef || !this.floorTextureRef) {
      return;
    }
    draw_ceiling_floor_raycast(
      player.toRustPosition(),
      this.ceilingFloorPixelsRef.ptr,
      this.ceilingFloorBlackPixelsRef.ptr,
      this.floorTextureRef.ptr,
      this.ceilingTextureRef.ptr,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution,
      this.lightRange,
      map.light,
      map.floorTexture.width,
      map.floorTexture.height,
      map.ceilingTexture.width,
      map.ceilingTexture.height,
      map.wallGrid, // 1D array instead of 2D
      map.size, // Width of original 2D array
      this.height
    );

    // scale image to canvas width/height
    var img0 = new ImageData(
      new Uint8ClampedArray(this.ceilingFloorBlackPixelsRef.buffer),
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer0 = await createImageBitmap(img0);
    this.ctx.drawImage(renderer0, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img0, 0, 0);

    // scale image to canvas width/height
    var img = new ImageData(
      new Uint8ClampedArray(this.ceilingFloorPixelsRef.buffer),
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer = await createImageBitmap(img);
    this.ctx.drawImage(renderer, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img, 0, 0);
  }

  drawWallsRaycastWasm(player: Player, map: GridMap): void {
    draw_walls_raycast(
      this.columnsRef.ptr,
      this.zBufferRef.ptr,
      player.toRustPosition(),
      map.wallGrid, // 1D array instead of 2D
      map.size, // Width of original 2D array
      this.widthResolution,
      this.height,
      this.width,
      this.widthSpacing,
      this.lightRange,
      this.range,
      map.wallTexture.width
    );
    let width = Math.ceil(this.widthSpacing);
    for (let idx = 0; idx < this.columnsRef.buffer.length / 7; idx += 7) {
      let [tex_x, left, draw_start_y, wall_height, global_alpha, hit] = [
        this.columnsRef.buffer[idx],
        this.columnsRef.buffer[idx + 1],
        this.columnsRef.buffer[idx + 2],
        this.columnsRef.buffer[idx + 3],
        this.columnsRef.buffer[idx + 4],
        this.columnsRef.buffer[idx + 5],
        this.columnsRef.buffer[idx + 6],
      ];

      if (hit) {
        this.ctx.drawImage(
          map.wallTexture.image,
          tex_x, // sx
          0, // sy
          1, // sw
          map.wallTexture.height, // sh
          left, // dx
          draw_start_y, // dy - yes we go into minus here, it'll be ignored anyway
          width, // dw
          wall_height // dh
        );
        this.ctx.globalAlpha = global_alpha / 100;
        this.ctx.fillRect(left, draw_start_y, width, wall_height);
        this.ctx.globalAlpha = 1;
      }
    }
  }

  // draws columns left to right
  drawSprites(sprites: Sprite[], player: Player, map: GridMap): void {
    // SPRITE CASTING
    // sort sprites from far to close
    const sortedSprites = sortBy(
      sprites,
      (sprite) =>
        (player.position.x - sprite.x) * (player.position.x - sprite.x) +
        (player.position.y - sprite.y) * (player.position.y - sprite.y)
    ).reverse();

    this.ctx.save();

    // after sorting the sprites, do the projection and draw them
    for (let i = 0; i < sprites.length; i++) {
      // // translate sprite position to relative to camera
      let sprite = sortedSprites[i];
      const { texture, spriteTextureHeight } = map.getSpriteTexture(sprite);

      const {
        screen_x: screenX,
        screen_y_ceiling: screenYCeiling,
        screen_y_floor: screenYFloor,
        distance,
        full_height: fullHeight,
      } = translate_coordinate_to_camera(
        player.toRustPosition(),
        sprite.x,
        sprite.y,
        spriteTextureHeight,
        this.width,
        this.height
      );

      // calculate width of the sprite
      let spriteWidth = Math.abs(
        Math.floor(fullHeight * (texture.width / texture.height))
      );
      let drawStartX = Math.floor(-spriteWidth / 2 + screenX);
      if (drawStartX < 0) drawStartX = 0;
      let drawEndX = spriteWidth / 2 + screenX;
      if (drawEndX >= this.width) drawEndX = this.width - 1;

      const alpha = distance / this.lightRange - map.light;
      // ensure sprites are always at least a little bit visible - alpha 1 is all black
      this.ctx.filter = `brightness(${Math.min(
        Math.max(0, Math.floor(100 - alpha * 100), 20)
      )}%)`; // min 20% brightness

      // push parts of stripe that are visible into array and draw in discrete steps (since brightness is very inefficient we cannot draw vertical stripe by vertical stripe)
      let stripeParts: number[] = [];
      for (
        let stripe = drawStartX;
        stripe < drawEndX;
        stripe += this.widthSpacing
      ) {
        // the conditions in the if are:
        //1) it's in front of camera plane so you don't see things behind you
        //2) it's on the screen (left)
        //3) it's on the screen (right)
        //4) ZBuffer, with perpendicular distance
        if (
          distance > 0 &&
          stripe >= 0 &&
          stripe <= this.width &&
          distance <
            this.zBufferRef.buffer[Math.floor(stripe / this.widthSpacing)]
        ) {
          // no x yet
          if (stripeParts.length % 2 === 0) {
            let dx = Math.floor(stripe);
            stripeParts.push(dx);
          }
          // handle last frame
          if (
            stripe + this.widthSpacing >= drawEndX &&
            stripeParts.length % 2 === 1
          ) {
            stripeParts.push(stripe);
          }
        } else if (stripeParts.length % 2 === 1) {
          // no y yet
          stripeParts.push(stripe);
        }
      }

      for (let stripeIdx = 0; stripeIdx < stripeParts.length; stripeIdx += 2) {
        const stripeLeftX = stripeParts[stripeIdx];
        const stripeRightX = stripeParts[stripeIdx + 1];
        let texX1 = Math.floor(
          ((stripeLeftX - (-spriteWidth / 2 + screenX)) * texture.width) /
            spriteWidth
        );
        let texX2 = Math.ceil(
          ((stripeRightX - (-spriteWidth / 2 + screenX)) * texture.width) /
            spriteWidth
        );

        this.ctx.drawImage(
          texture.image,
          texX1, // sx
          0, // sy
          texX2 - texX1, // sw
          texture.height, // sh
          stripeLeftX, // dx
          screenYCeiling, // dy
          stripeRightX - stripeLeftX, // dw
          screenYFloor - screenYCeiling // dh
        );
      }
    }
    this.ctx.restore();
  }

  // draws columns left to right
  async drawColumns(player: Player, map: GridMap, spriteMap: SpriteMap) {
    this.ctx.save();

    const { coords, sprites } = this.raycastVisibleCoordinatesWasm(
      spriteMap,
      player,
      map
    );
    await this.drawCeilingFloorRaycastWasm(player, map);
    this.drawWallsRaycastWasm(player, map);
    this.drawSprites(sprites, player, map);

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
