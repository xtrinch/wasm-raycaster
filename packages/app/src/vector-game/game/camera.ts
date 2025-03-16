import { flatten, sortBy } from "lodash";
import { makeAutoObservable } from "mobx";
import {
  allocate_u8_array,
  draw_ceiling_floor_raycast,
  raycast_visible_coordinates,
  WasmUint8Array,
} from "../../../wasm";
import { memory } from "../../../wasm/index_bg.wasm";
import { draw_walls_raycast } from "../../wasm";
import { drawArbitraryQuadImage, FILL_METHOD } from "../arbitrary-quads";
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

interface Sprite {
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
  public floorData: Uint8ClampedArray<ArrayBufferLike>;
  public ceilingData: Uint8ClampedArray<ArrayBufferLike>;
  public originalCanvas: HTMLCanvasElement;
  public myWasmArrayPtr: number;
  public myWasmArray: Uint8Array;
  public f: WasmUint8Array;
  public myWasmArrayPtr1: number;
  public myWasmArray1: Uint8Array;
  public f1: WasmUint8Array;

  constructor(canvas: HTMLCanvasElement, map: GridMap) {
    this.ctx = canvas.getContext("2d");
    this.width = canvas.width = window.innerWidth;
    this.height = canvas.height = window.innerHeight;
    this.widthResolution = this.width; //620;
    this.heightResolution = 420;
    const factor = 1 / 2;
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
    this.intializeTexture(this.map.floorTexture, "floorData");
    this.intializeTexture(this.map.ceilingTexture, "ceilingData");
    // Preemptively grow memory (10 * 64KB = 640KB)
    // memory.grow(1000);

    let length = this.ceilingWidthResolution * this.ceilingHeightResolution * 4;
    // Allocate memory in WebAssembly
    console.log("Memory buffer before:", memory.buffer.byteLength);

    this.myWasmArrayPtr = allocate_u8_array(length);

    console.log("Memory buffer after:", memory.buffer.byteLength);
    this.myWasmArray = new Uint8Array(
      memory.buffer,
      this.myWasmArrayPtr,
      length
    );

    const f = new WasmUint8Array(length);
    this.f = f;
    this.myWasmArray = f.buffer;
    this.myWasmArrayPtr = f.ptr;

    const f1 = new WasmUint8Array(length);
    this.f1 = f1;
    this.myWasmArray1 = f1.buffer;
    this.myWasmArrayPtr1 = f1.ptr;
    makeAutoObservable(this);
  }

  intializeTexture(texture: Bitmap, key: string) {
    const img = texture.image;
    const canvas = document.createElement("canvas") as HTMLCanvasElement;
    this.context = canvas.getContext("2d");
    canvas.width = texture.width * 2;
    canvas.height = texture.height * 2;
    texture.image.onload = () => {
      this.context.drawImage(img, 0, 0, texture.width, texture.height);
      this[key] = this.context.getImageData(
        0,
        0,
        texture.width,
        texture.height
      )?.data;
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
  translateCoordinateToCamera(
    player: Player,
    point: number[],
    heightMultiplier: number = 1,
    absNegative: boolean = true
  ): {
    screenX: number;
    screenYFloor: number;
    screenYCeiling: number;
    distance: number;
    fullHeight: number;
    transformX: number;
    transformY: number;
  } {
    let playerPositionX = player.position.x;
    let playerPositionY = player.position.y;
    let playerPositionPlaneX = player.position.planeX;
    let playerPositionPlaneY = player.position.planeY;
    let playerPositionDirX = player.position.dirX;
    let playerPositionDirY = player.position.dirY;

    // translate x y position to relative to camera
    let spriteX = point[0] - playerPositionX; //+ player.position.dirX;
    let spriteY = point[1] - playerPositionY; //+ player.position.dirY;

    // transform sprite with the inverse camera matrix
    // [ planeX   dirX ] -1                                       [ dirY      -dirX ]
    // [               ]       =  1/(planeX*dirY-dirX*planeY) *   [                 ]
    // [ planeY   dirY ]                                          [ -planeY  planeX ]

    let invDet = Math.abs(
      1.0 /
        (playerPositionPlaneX * playerPositionDirY -
          playerPositionDirX * playerPositionPlaneY)
    ); // required for correct matrix multiplication
    let transformX =
      invDet * (playerPositionDirY * spriteX - playerPositionDirX * spriteY);
    let transformY =
      invDet *
      (-playerPositionPlaneY * spriteX + playerPositionPlaneX * spriteY); // this is actually the depth inside the screen, that what Z is in 3D

    if (transformY < 0.3 && absNegative) {
      transformY = 0.3;
    }
    if (transformY < 0.01) {
      // transformY = 0.01;
    }

    let screenX = Math.floor((this.width / 2) * (1 + transformX / transformY));

    // to control the pitch / jump
    let vMoveScreen = player.position.pitch + player.position.z;

    // DIVIDE BY FOCAL LENGTH WHICH IS LENGTH OF THE PLANE VECTOR
    let yHeightBeforeAdjustment = Math.abs(
      Math.floor(this.width / 2 / player.position.planeYInitial / transformY)
    );
    let yHeight = yHeightBeforeAdjustment * heightMultiplier; // using 'transformY' instead of the real distance prevents fisheye
    let heightDistance = yHeightBeforeAdjustment - yHeight;
    let screenCeilingY = this.height / 2 - yHeight / 2;
    let screenFloorY = this.height / 2 + yHeight / 2;
    let spriteFloorScreenY = screenFloorY + vMoveScreen + heightDistance / 2;
    let spriteCeilingScreenY =
      screenCeilingY + vMoveScreen + heightDistance / 2;
    let fullHeight = spriteCeilingScreenY - spriteFloorScreenY;

    return {
      screenX: screenX,
      screenYFloor: spriteFloorScreenY,
      screenYCeiling: spriteCeilingScreenY,
      distance: transformY,
      fullHeight: fullHeight,
      transformX,
      transformY,
    };
  }

  raycastVisibleCoordinatesWasm(
    spriteMap: SpriteMap,
    player: Player,
    map: GridMap
  ): { coords: Coords; sprites: Sprite[] } {
    const data: { coords: Map<any, any>; sprites: Sprite[] } =
      raycast_visible_coordinates(
        this.widthResolution,
        this.range,
        player.position.x,
        player.position.y,
        player.position.dirX,
        player.position.dirY,
        player.position.planeX,
        player.position.planeY,
        map.wallGrid, // 1D array instead of 2D
        map.size, // Width of original 2D array
        new Float64Array(flatten(spriteMap.sprites))
      );
    return { sprites: data.sprites, coords: Object.fromEntries(data.coords) };
  }

  raycastVisibleCoordinates(
    spriteMap: SpriteMap,
    player: Player,
    map: GridMap
  ): { coords: Coords; sprites: Sprite[] } {
    const coords: Coords = {};
    const sprites: Sprite[] = [];

    // wall casting
    for (let column = 0; column < this.widthResolution; column++) {
      // x-coordinate in camera space scaled from -1 to 1
      let cameraX = (2 * column) / this.widthResolution - 1;
      // get the ray direction vector
      let rayDirX = player.position.dirX + player.position.planeX * cameraX;
      let rayDirY = player.position.dirY + player.position.planeY * cameraX;

      // which box of the map we're in
      let mapX = Math.floor(player.position.x);
      let mapY = Math.floor(player.position.y);

      // length of ray from current position to next x or y-side
      let sideDistX: number;
      let sideDistY: number;

      //length of ray from one x or y-side to next x or y-side
      //these are derived as:
      //deltaDistX = sqrt(1 + (rayDirY * rayDirY) / (rayDirX * rayDirX))
      //deltaDistY = sqrt(1 + (rayDirX * rayDirX) / (rayDirY * rayDirY))
      //which can be simplified to abs(|rayDir| / rayDirX) and abs(|rayDir| / rayDirY)
      //where |rayDir| is the length of the vector (rayDirX, rayDirY). Its length,
      //unlike (dirX, dirY) is not 1, however this does not matter, only the
      //ratio between deltaDistX and deltaDistY matters, due to the way the DDA
      //stepping further below works. So the values can be computed as below.
      // Division through zero is prevented
      let deltaDistX = Math.abs(1 / rayDirX);
      let deltaDistY = Math.abs(1 / rayDirY);
      // let deltaDistX =
      //   rayDirX === 0 ? 1e30 : Math.sqrt(1 + rayDirY ** 2 / rayDirX ** 2);
      // let deltaDistY =
      //   rayDirY === 0 ? 1e30 : Math.sqrt(1 + rayDirX ** 2 / rayDirY ** 2);

      // perpendicular wall distance
      let perpWallDist: number;

      // what direction to step in x or y-direction (either +1 or -1)
      let stepX: number;
      let stepY: number;

      let hit: number = 0; // was there a wall hit?

      // calculate step and initial sideDist
      if (rayDirX < 0) {
        stepX = -1;
        sideDistX = (player.position.x - mapX) * deltaDistX;
      } else {
        stepX = 1;
        sideDistX = (mapX + 1.0 - player.position.x) * deltaDistX;
      }

      if (rayDirY < 0) {
        stepY = -1;
        sideDistY = (player.position.y - mapY) * deltaDistY;
      } else {
        stepY = 1;
        sideDistY = (mapY + 1.0 - player.position.y) * deltaDistY;
      }

      let side: number; // was a NS or a EW wall hit? if x then side = 0, if y then side = 1
      // perform DDA
      let range = this.range;
      while (hit == 0 && range >= 0) {
        // // jump to next map square, either in x-direction, or in y-direction

        const xCoord = Math.floor(mapX);
        const yCoord = Math.floor(mapY);
        const mapValue = map.get(mapX, mapY);
        // Check if ray has hit a wall
        if (mapValue == 1) hit = 1;
        const hasCeilingFloor = mapValue === 2;
        const coordIdx = `${xCoord}-${yCoord}}`;
        const oldCoordObj = coords[coordIdx];

        // only once so we don't duplicate the sprites
        if (!oldCoordObj) {
          // find sprites
          const spritesForCoord = spriteMap.sprites
            .filter(
              (sp) =>
                Math.floor(sp[0]) === xCoord && Math.floor(sp[1]) === yCoord
            )
            .map((sp) => ({ x: sp[0], y: sp[1], type: sp[2] }));
          sprites.push(...spritesForCoord);
        }

        let coordObj = oldCoordObj || { visibleSquares: {} };
        coords[`${xCoord}-${yCoord}}`] = {
          ...coordObj,
          distance: 0,
          hasWall: !!hit,
          x: xCoord,
          y: yCoord,
          hasCeilingFloor: hasCeilingFloor,
        };
        if (hit) {
          // bottom two coords of the side of the wall we're looking at
          let x: number, y: number, x1: number, y1: number;
          let wallSide: "n" | "e" | "w" | "s" = null;
          if (stepX === 1 && stepY === 1) {
            if (side === 0) {
              wallSide = "w";
            } else if (side === 1) {
              wallSide = "s";
            }
          } else if (stepX === -1 && stepY === 1) {
            if (side === 0) {
              wallSide = "e";
            } else if (side === 1) {
              wallSide = "s";
            }
          } else if (stepX === 1 && stepY === -1) {
            if (side === 0) {
              wallSide = "w";
            } else if (side === 1) {
              wallSide = "n";
            }
          } else if (stepX === -1 && stepY === -1) {
            if (side === 0) {
              wallSide = "e";
            } else if (side === 1) {
              wallSide = "n";
            }
          }

          switch (wallSide) {
            case "n":
              x = xCoord;
              y = yCoord + 1;
              x1 = xCoord + 1;
              y1 = yCoord + 1;
              break;
            case "s":
              x = xCoord;
              y = yCoord;
              x1 = xCoord + 1;
              y1 = yCoord;
              break;
            case "e":
              x = xCoord + 1;
              y = yCoord;
              x1 = xCoord + 1;
              y1 = yCoord + 1;
              break;
            case "w":
              x = xCoord;
              y = yCoord;
              x1 = xCoord;
              y1 = yCoord + 1;
              break;
          }
          // add the visible square we're seeing
          if (!coords[coordIdx].visibleSquares[wallSide]) {
            coords[coordIdx].visibleSquares[wallSide] = [x, y, x1, y1];
          }
        }

        // jump to next map square, either in x-direction, or in y-direction
        if (sideDistX < sideDistY) {
          sideDistX += deltaDistX;
          mapX += stepX;
          side = 0;
        } else {
          sideDistY += deltaDistY;
          mapY += stepY;
          side = 1;
        }
        range -= 1;
      }
      // Calculate distance projected on camera direction. This is the shortest distance from the point where the wall is
      // hit to the camera plane. Euclidean to center camera plane would give fisheye effect!
      // This can be computed as (mapX - posX + (1 - stepX) / 2) / rayDirX for side == 0, or same formula with Y
      // for size == 1, but can be simplified to the code below thanks to how sideDist and deltaDist are computed:
      // because they were left scaled to |rayDir|. sideDist is the entire length of the ray above after the multiple
      // steps, but we subtract deltaDist once because one step more into the wall was taken above.
      if (side == 0) perpWallDist = sideDistX - deltaDistX;
      else perpWallDist = sideDistY - deltaDistY;
      // if (side === 0)
      //   perpWallDist = (mapX - player.position.x + (1 - stepX) / 2) / rayDirX;
      // else
      //   perpWallDist = (mapY - player.position.y + (1 - stepY) / 2) / rayDirY;

      let texture = map.wallTexture;

      // calculate value of wallX
      let wallX: number; // where exactly the wall was hit
      if (side == 0) wallX = player.position.y + perpWallDist * rayDirY;
      else wallX = player.position.x + perpWallDist * rayDirX;
      wallX -= Math.floor(wallX);

      // x coordinate on the texture
      let texX = Math.floor(wallX * texture.width);
      if (side == 0 && rayDirX > 0) texX = texture.width - texX - 1;
      if (side == 1 && rayDirY < 0) texX = texture.width - texX - 1;

      this.ctx.globalAlpha = 1;
    }

    return { coords, sprites };
  }

  drawWallQuad(coords: Coords, player: Player, map: GridMap) {
    const wallSrcPoints = [
      { x: 0, y: 0 },
      { x: 0, y: map.wallTexture.height },
      { x: map.wallTexture.width, y: map.wallTexture.height },
      { x: map.wallTexture.width, y: 0 },
    ];

    const coordValues = Object.values(coords);
    const squares: {
      avgDistance: number;
      screenX1: number;
      screenYFloor1: number;
      screenYCeiling1: number;
      screenX2: number;
      screenYFloor2: number;
      screenYCeiling2: number;
      screenXHalf: number;
      screenYFloorHalf: number;
      screenYCeilingHalf: number;
      key: string;
      x: number;
      y: number;
      transformX1: number;
      transformX2: number;
      transformY1: number;
      transformY2: number;
    }[] = [];
    // for each coordinate that we see on screen
    for (let coordItem of coordValues) {
      for (let [key, wall] of Object.entries(coordItem.visibleSquares)) {
        const {
          screenX: screenX1,
          screenYFloor: screenYFloor1,
          screenYCeiling: screenYCeiling1,
          distance: x1Distance,
          transformX: transformX1,
          transformY: transformY1,
        } = this.translateCoordinateToCamera(player, [wall[0], wall[1]]);
        const {
          screenX: screenXHalf,
          screenYFloor: screenYFloorHalf,
          screenYCeiling: screenYCeilingHalf,
        } = this.translateCoordinateToCamera(player, [
          (wall[0] + wall[2]) / 2,
          (wall[1] + wall[3]) / 2,
        ]);
        const {
          screenX: screenX2,
          screenYFloor: screenYFloor2,
          screenYCeiling: screenYCeiling2,
          distance: x2Distance,
          transformX: transformX2,
          transformY: transformY2,
        } = this.translateCoordinateToCamera(player, [wall[2], wall[3]]);
        const avgDistance = (x1Distance + x2Distance) / 2;

        squares.push({
          x: wall[0],
          y: wall[1],
          avgDistance,
          screenX1,
          screenYFloor1,
          screenYCeiling1,
          screenX2,
          screenYFloor2,
          screenYCeiling2,
          screenXHalf,
          screenYFloorHalf,
          screenYCeilingHalf,
          key,
          transformX1: transformX1,
          transformY1: transformY1,
          transformX2: transformX2,
          transformY2: transformY2,
        });
      }
    }
    const sortedSquares = sortBy(
      squares,
      (sprite) => sprite.avgDistance
    ).reverse();
    let idx = 0;
    for (let square of sortedSquares) {
      idx += 1;
      this.ctx.save();
      // 1st half
      drawArbitraryQuadImage(
        this.ctx,
        this.map.wallTexture.image as CanvasImageSource,
        wallSrcPoints,
        [
          { x: square.screenX2, y: square.screenYCeiling2 },
          { x: square.screenX2, y: square.screenYFloor2 },
          { x: square.screenX1, y: square.screenYFloor1 },
          { x: square.screenX1, y: square.screenYCeiling1 },
        ],
        FILL_METHOD.BILINEAR,
        4
      );

      this.ctx.restore();

      // handle brightness
      this.ctx.save();

      const alpha = square.avgDistance / this.lightRange - map.light;
      // ensure walls are always at least a little bit visible - alpha 1 is all black
      let brightness = Math.min(Math.max(0, Math.floor(100 - alpha * 100), 20));
      // brightness = 0;
      this.ctx.fillStyle = `rgba(0, 0, 0, ${1 - brightness / 100})`;

      this.ctx.beginPath();
      this.ctx.moveTo(square.screenX2 || 0, square.screenYCeiling2 || 0);
      this.ctx.lineTo(square.screenX2 || 0, square.screenYFloor2 || 0);
      this.ctx.lineTo(square.screenX1 || 0, square.screenYFloor1 || 0);
      this.ctx.lineTo(square.screenX1 || 0, square.screenYCeiling1 || 0);
      this.ctx.fill();

      this.ctx.strokeStyle = `rgb(255, 255, 255)`;
      this.ctx.beginPath();
      this.ctx.moveTo(square.screenX2 || 0, square.screenYCeiling2 || 0);
      this.ctx.lineTo(square.screenX2 || 0, square.screenYFloor2 || 0);
      this.ctx.lineTo(square.screenX1 || 0, square.screenYFloor1 || 0);
      this.ctx.lineTo(square.screenX1 || 0, square.screenYCeiling1 || 0);
      this.ctx.stroke();

      this.ctx.fillStyle = "#000000";
      this.ctx.font = "20px serif";
      this.ctx.fillText(
        `${square.screenX1}/${square.screenX2}`,
        (square.screenX1 + square.screenX2) / 2,
        this.height / 2
      );
      this.ctx.fillText(
        `${square.screenX1}/${square.screenX2}`,
        (square.screenX1 + square.screenX2) / 2 +
          (square.screenX2 - square.screenX1) / 4,
        this.height / 2
      );
      this.ctx.fillText(
        `${square.screenX1}/${square.screenX2}`,
        square.screenX1,
        this.height / 2
      );
      this.ctx.fillText(
        `tx1:${square.transformX1.toFixed(3)}`,
        square.screenX1,
        this.height / 2 + 30
      );
      this.ctx.fillText(
        `ty1:${square.transformY1.toFixed(3)}`,
        square.screenX1,
        this.height / 2 + 50
      );
      this.ctx.fillText(
        `tx2: ${square.transformX2.toFixed(3)} `,
        square.screenX1,
        this.height / 2 + 70
      );
      this.ctx.fillText(
        `ty2: ${square.transformY2.toFixed(3)}`,
        square.screenX1,
        this.height / 2 + 90
      );
      this.ctx.fillText(
        `key: ${square.key}`,
        square.screenX1,
        this.height / 2 + 110
      );
      this.ctx.restore();
    }
  }

  async drawCeilingFloorRaycastWasm(player: Player, map: GridMap) {
    const data: {
      black_pixels: Uint8Array<ArrayBuffer>;
      texture_pixels: Uint8Array<ArrayBuffer>;
    } = draw_ceiling_floor_raycast(
      this.myWasmArrayPtr,
      this.myWasmArrayPtr1,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution,
      this.lightRange,
      map.light,
      player.position.x,
      player.position.y,
      player.position.dirX,
      player.position.dirY,
      player.position.planeX,
      player.position.planeY,
      player.position.pitch,
      player.position.z,
      new Uint8Array(this.floorData),
      map.floorTexture.width,
      map.floorTexture.height,
      new Uint8Array(this.ceilingData),
      map.ceilingTexture.width,
      map.ceilingTexture.height,
      map.wallGrid, // 1D array instead of 2D
      map.size, // Width of original 2D array
      map.size,
      this.height
    );

    this.myWasmArray = this.f.buffer;
    this.myWasmArrayPtr = this.f.ptr;

    this.myWasmArray1 = this.f1.buffer;
    this.myWasmArrayPtr1 = this.f1.ptr;
    // return;

    // scale image to canvas width/height
    var img0 = new ImageData(
      new Uint8ClampedArray(this.myWasmArray1),
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer0 = await createImageBitmap(img0);
    this.ctx.drawImage(renderer0, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img0, 0, 0);

    // scale image to canvas width/height
    var img = new ImageData(
      // new Uint8ClampedArray(data.texture_pixels),
      new Uint8ClampedArray(this.myWasmArray),
      // this.myWasmArray as any as Uint8ClampedArray,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer = await createImageBitmap(img);
    this.ctx.drawImage(renderer, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img, 0, 0);
  }

  // draws columns left to right
  async drawCeilingFloorRaycast(player: Player, map: GridMap) {
    if (!this.floorData || !this.ceilingData) {
      return;
    }

    const floorTexture = map.floorTexture;
    const ceilingTexture = map.ceilingTexture;
    const floorData = this.floorData;
    const ceilingData = this.ceilingData;

    // floor casting
    const ceilingFloorImg = new Uint8ClampedArray(
      this.ceilingWidthResolution * this.ceilingHeightResolution * 4
    );
    // black pixels for floor so we can use alpha channel in the actual floor
    const floorImgBlackPixels = new Uint8ClampedArray(
      this.ceilingWidthResolution * this.ceilingHeightResolution * 4
    );

    // rayDir for leftmost ray (x = 0) and rightmost ray (x = w)
    let rayDirX0 = player.position.dirX - player.position.planeX;
    let rayDirY0 = player.position.dirY - player.position.planeY;
    let rayDirX1 = player.position.dirX + player.position.planeX;
    let rayDirY1 = player.position.dirY + player.position.planeY;

    const rayDirXDist = rayDirX1 - rayDirX0;
    const rayDirYDist = rayDirY1 - rayDirY0;

    // Vertical position of the camera.
    let halfHeight = this.ceilingHeightResolution / 2;

    // since we're drawing a smaller image, we also need to scale the pitch
    const scale = this.ceilingHeightResolution / this.height;
    let scaledPitch = player.position.pitch * scale;
    let scaledZ = player.position.z * scale;

    // loop through the resolutions and scale later
    for (let y = 0; y < this.ceilingHeightResolution; ++y) {
      // whether this section is floor or ceiling
      let isFloor = y > halfHeight + scaledPitch;

      // Current y position compared to the center of the screen (the horizon)
      let p = isFloor
        ? y - halfHeight - scaledPitch
        : halfHeight - y + scaledPitch;

      // Vertical position of the camera.
      // NOTE: with 0.5, it's exactly in the center between floor and ceiling,
      // matching also how the walls are being raycasted. For different values
      // than 0.5, a separate loop must be done for ceiling and floor since
      // they're no longer symmetrical.
      let camZ = isFloor ? halfHeight + scaledZ : halfHeight - scaledZ;

      // Horizontal distance from the camera to the floor for the current row.
      // 0.5 is the z position exactly in the middle between floor and ceiling.
      // NOTE: this is affine texture mapping, which is not perspective correct
      // except for perfectly horizontal and vertical surfaces like the floor.
      // NOTE: this formula is explained as follows: The camera ray goes through
      // the following two points: the camera itself, which is at a certain
      // height (posZ), and a point in front of the camera (through an imagined
      // vertical plane containing the screen pixels) with horizontal distance
      // 1 from the camera, and vertical position p lower than posZ (posZ - p). When going
      // through that point, the line has vertically traveled by p units and
      // horizontally by 1 unit. To hit the floor, it instead needs to travel by
      // posZ units. It will travel the same ratio horizontally. The ratio was
      // 1 / p for going through the camera plane, so to go posZ times farther
      // to reach the floor, we get that the total horizontal distance is posZ / p.
      let rowDistance = camZ / p;

      let alpha = (rowDistance + 0) / this.lightRange - map.light;
      alpha = Math.min(alpha, 0.8);

      // calculate the real world step vector we have to add for each x (parallel to camera plane)
      // adding step by step avoids multiplications with a weight in the inner loop
      let floorStepX =
        (rowDistance * rayDirXDist) / this.ceilingWidthResolution;
      let floorStepY =
        (rowDistance * rayDirYDist) / this.ceilingWidthResolution;

      // real world coordinates of the leftmost column. This will be updated as we step to the right.
      let floorX = player.position.x + rowDistance * rayDirX0;
      let floorY = player.position.y + rowDistance * rayDirY0;

      const rowAlpha = (1 - alpha) * 255;
      for (let x = 0; x < this.ceilingWidthResolution; ++x) {
        floorX += floorStepX;
        floorY += floorStepY;

        // no roof, no draw
        if (map.get(floorX, floorY) !== 2) continue;

        let cellX = floorX % 1; // the cell coord is simply got from the integer parts of floorX and floorY
        let cellY = floorY % 1;

        let textureWidth = isFloor ? floorTexture.width : ceilingTexture.width;
        let textureHeight = isFloor
          ? floorTexture.height
          : ceilingTexture.height;

        // get the texture coordinate from the fractional part
        let tx = Math.floor(textureWidth * cellX);
        let ty = Math.floor(textureHeight * cellY);

        // find pixel
        const fullImgIdx = 4 * (ty * textureWidth + tx);
        const sliceFloor = floorData.slice(fullImgIdx, fullImgIdx + 4);
        const sliceCeiling = ceilingData.slice(fullImgIdx, fullImgIdx + 4);

        sliceFloor[3] = rowAlpha;
        sliceCeiling[3] = rowAlpha;
        const floorImgIdx = 4 * (y * this.ceilingWidthResolution + x);

        if (isFloor) {
          ceilingFloorImg.set(sliceFloor, floorImgIdx);
          floorImgBlackPixels.set([0, 0, 0, 255], floorImgIdx);
        } else {
          ceilingFloorImg.set(sliceCeiling, floorImgIdx);
          floorImgBlackPixels.set([0, 0, 0, 255], floorImgIdx);
        }
      }
    }

    // scale image to canvas width/height
    var img0 = new ImageData(
      floorImgBlackPixels,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer0 = await createImageBitmap(img0);
    this.ctx.drawImage(renderer0, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img0, 0, 0);

    // scale image to canvas width/height
    var img = new ImageData(
      ceilingFloorImg,
      this.ceilingWidthResolution,
      this.ceilingHeightResolution
    );

    const renderer = await createImageBitmap(img);
    this.ctx.drawImage(renderer, 0, 0, this.width, this.height);
    // this.ctx.putImageData(img, 0, 0);
  }

  // draws columns left to right
  drawCeilingFloorTexture(coords: Coords, player: Player, map: GridMap) {
    const ceilingSrcPoints = [
      { x: 0, y: 0 },
      { x: 0, y: map.ceilingTexture.height },
      { x: map.ceilingTexture.width, y: map.ceilingTexture.height },
      { x: map.ceilingTexture.width, y: 0 },
    ];
    const floorSrcPoints = [
      { x: 0, y: 0 },
      { x: 0, y: map.floorTexture.height },
      { x: map.floorTexture.width, y: map.floorTexture.height },
      { x: map.floorTexture.width, y: 0 },
    ];
    // get top left, top right, bottom right, bottom left x y coord
    // find which x and ys are on the screen

    const width = this.width;
    let dst = 0;
    const coordValues = Object.values(coords);

    // for each coordinate that we see on screen
    for (let coordItem of coordValues) {
      const x = coordItem.x;
      const y = coordItem.y;
      let points = [
        [x, y],
        [x + 1.01, y],
        [x + 1.01, y + 1.01],
        [x, y + 1.01],
      ];
      let floorScreenPoints: { x: number; y: number }[] = [];
      let ceilingScreenPoints: { x: number; y: number }[] = [];
      let numOnScreen = 0;
      if (map.get(x, y) !== 2) {
        continue;
      }
      for (let point of points) {
        const { screenX, screenYCeiling, screenYFloor, distance } =
          this.translateCoordinateToCamera(player, point);

        if (
          distance >= 0 &&
          screenX >= 0 &&
          screenX <= width &&
          distance < 10
        ) {
          dst = Math.abs(distance);
          numOnScreen++;
        }
        floorScreenPoints.push({ x: screenX, y: screenYFloor });
        ceilingScreenPoints.push({
          x: screenX,
          y: screenYCeiling,
        });
      }

      let tesselation = 9 - dst;
      if (tesselation <= 1) tesselation = 1;
      if (tesselation >= 7) tesselation = 7;
      if (numOnScreen >= 1) {
        const alpha = (dst + 0) / this.lightRange - map.light;
        // ensure walls are always at least a little bit visible - alpha 1 is all black
        const brightness = Math.min(
          Math.max(0, Math.floor(100 - alpha * 100), 20)
        );
        this.ctx.save();
        drawArbitraryQuadImage(
          this.ctx,
          this.map.floorTexture.image as CanvasImageSource,
          floorSrcPoints,
          floorScreenPoints,
          FILL_METHOD.BILINEAR,
          tesselation
        );
        drawArbitraryQuadImage(
          this.ctx,
          this.map.ceilingTexture.image as CanvasImageSource,
          ceilingSrcPoints,
          ceilingScreenPoints,
          FILL_METHOD.BILINEAR,
          tesselation
        );
        this.ctx.restore();

        // handle brightness
        this.ctx.save();
        this.ctx.fillStyle = `rgba(0, 0, 0, ${1 - brightness / 100})`;

        this.ctx.beginPath();
        this.ctx.moveTo(ceilingScreenPoints[0].x, ceilingScreenPoints[0].y);
        this.ctx.lineTo(ceilingScreenPoints[1].x, ceilingScreenPoints[1].y);
        this.ctx.lineTo(ceilingScreenPoints[2].x, ceilingScreenPoints[2].y);
        this.ctx.lineTo(ceilingScreenPoints[3].x, ceilingScreenPoints[3].y);
        this.ctx.fill();

        this.ctx.beginPath();
        this.ctx.moveTo(floorScreenPoints[0].x, floorScreenPoints[0].y);
        this.ctx.lineTo(floorScreenPoints[1].x, floorScreenPoints[1].y);
        this.ctx.lineTo(floorScreenPoints[2].x, floorScreenPoints[2].y);
        this.ctx.lineTo(floorScreenPoints[3].x, floorScreenPoints[3].y);
        this.ctx.fill();

        this.ctx.restore();
      }
    }
  }

  // draws columns left to right
  drawCeilingFloorNoTexture(coords: Coords, player: Player, map: GridMap) {
    // get top left, top right, bottom right, bottom left x y coord
    // find which x and ys are on the screen

    const width = this.width;
    let dst = 0;
    const coordValues = Object.values(coords);

    // for each coordinate that we see on screen
    for (let coordItem of coordValues) {
      const x = coordItem.x;
      const y = coordItem.y;
      let points = [];
      let lenX = 3;
      let lenY = 3;
      let pointX = 0;
      let pointY = 0;
      for (pointX = 0; pointX < lenX; pointX++) {
        for (pointY = 0; pointY < lenY; pointY++) {
          const xStep0 = (1 / lenX) * pointX;
          const yStep0 = (1 / lenY) * pointY;
          const xStep1 = 1 / lenX;
          const yStep1 = 1 / lenY;
          points = [
            [x + xStep0, y + yStep0],
            [x + xStep0 + xStep1, y + yStep0],
            [x + xStep0 + xStep1, y + yStep0 + yStep1],
            [x + xStep0, y + yStep0 + yStep1],
          ];

          let floorScreenPoints: { x: number; y: number }[] = [];
          let ceilingScreenPoints: { x: number; y: number }[] = [];
          let numOnScreen = 0;
          if (map.get(x, y) !== 2) {
            continue;
          }
          for (let point of points) {
            const { screenX, screenYCeiling, screenYFloor, distance } =
              this.translateCoordinateToCamera(player, point);

            if (
              distance >= 0 &&
              screenX >= 0 &&
              screenX <= width &&
              distance < this.range
            ) {
              dst = Math.abs(distance);
              numOnScreen++;
            }
            floorScreenPoints.push({ x: screenX, y: screenYFloor });
            ceilingScreenPoints.push({
              x: screenX,
              y: screenYCeiling,
            });
          }

          let tesselation = 9 - dst;
          if (tesselation <= 1) tesselation = 1;
          if (tesselation >= 7) tesselation = 7;
          if (numOnScreen >= 1) {
            const alpha = (dst + 0) / this.lightRange - map.light;
            // ensure walls are always at least a little bit visible - alpha 1 is all black
            const brightness = Math.min(
              Math.max(0, Math.floor(100 - alpha * 100), 20)
            );

            this.ctx.save();
            this.ctx.fillStyle = `rgb(65, 65, 65)`;
            this.ctx.beginPath();
            this.ctx.moveTo(floorScreenPoints[0].x, floorScreenPoints[0].y);
            this.ctx.lineTo(floorScreenPoints[1].x, floorScreenPoints[1].y);
            this.ctx.lineTo(floorScreenPoints[2].x, floorScreenPoints[2].y);
            this.ctx.lineTo(floorScreenPoints[3].x, floorScreenPoints[3].y);
            this.ctx.fill();

            this.ctx.fillStyle = `rgb(105, 105, 105)`;
            this.ctx.beginPath();
            this.ctx.moveTo(ceilingScreenPoints[0].x, ceilingScreenPoints[0].y);
            this.ctx.lineTo(ceilingScreenPoints[1].x, ceilingScreenPoints[1].y);
            this.ctx.lineTo(ceilingScreenPoints[2].x, ceilingScreenPoints[2].y);
            this.ctx.lineTo(ceilingScreenPoints[3].x, ceilingScreenPoints[3].y);
            this.ctx.fill();

            this.ctx.lineWidth = 3;

            this.ctx.strokeStyle = `rgba(0, 0, 0, 0.2)`;
            this.ctx.beginPath();
            this.ctx.moveTo(floorScreenPoints[0].x, floorScreenPoints[0].y);
            this.ctx.lineTo(floorScreenPoints[1].x, floorScreenPoints[1].y);
            this.ctx.lineTo(floorScreenPoints[2].x, floorScreenPoints[2].y);
            this.ctx.lineTo(floorScreenPoints[3].x, floorScreenPoints[3].y);
            this.ctx.stroke();

            this.ctx.strokeStyle = `rgba(0, 0, 0, 0.2)`;
            this.ctx.beginPath();
            this.ctx.moveTo(ceilingScreenPoints[0].x, ceilingScreenPoints[0].y);
            this.ctx.lineTo(ceilingScreenPoints[1].x, ceilingScreenPoints[1].y);
            this.ctx.lineTo(ceilingScreenPoints[2].x, ceilingScreenPoints[2].y);
            this.ctx.lineTo(ceilingScreenPoints[3].x, ceilingScreenPoints[3].y);
            this.ctx.stroke();

            // handle brightness
            this.ctx.fillStyle = `rgba(0, 0, 0, ${1 - brightness / 100})`;

            this.ctx.beginPath();
            this.ctx.moveTo(ceilingScreenPoints[0].x, ceilingScreenPoints[0].y);
            this.ctx.lineTo(ceilingScreenPoints[1].x, ceilingScreenPoints[1].y);
            this.ctx.lineTo(ceilingScreenPoints[2].x, ceilingScreenPoints[2].y);
            this.ctx.lineTo(ceilingScreenPoints[3].x, ceilingScreenPoints[3].y);
            this.ctx.fill();

            this.ctx.beginPath();
            this.ctx.moveTo(floorScreenPoints[0].x, floorScreenPoints[0].y);
            this.ctx.lineTo(floorScreenPoints[1].x, floorScreenPoints[1].y);
            this.ctx.lineTo(floorScreenPoints[2].x, floorScreenPoints[2].y);
            this.ctx.lineTo(floorScreenPoints[3].x, floorScreenPoints[3].y);
            this.ctx.fill();

            this.ctx.restore();
          }
        }
      }
    }
  }

  drawWallsRaycastWasm(
    player: Player,
    map: GridMap
  ): { [key: number]: number } {
    const columns = draw_walls_raycast(
      player.position.x,
      player.position.y,
      player.position.dirX,
      player.position.dirY,
      player.position.planeX,
      player.position.planeY,
      map.wallGrid, // 1D array instead of 2D
      map.size, // Width of original 2D array
      this.widthResolution,
      this.height,
      this.width,
      this.widthSpacing,
      this.lightRange,
      this.range,
      player.position.planeYInitial,
      player.position.pitch,
      player.position.z,
      map.wallTexture.width
    );
    let width = Math.ceil(this.widthSpacing);
    for (let idx = 0; idx < columns.length; idx++) {
      let column = columns[idx];
      if (column.hit) {
        this.ctx.drawImage(
          map.wallTexture.image,
          column.tex_x, // sx
          0, // sy
          1, // sw
          map.wallTexture.height, // sh
          column.left, // dx
          column.draw_start_y, // dy - yes we go into minus here, it'll be ignored anyway
          width, // dw
          column.wall_height // dh
        );
        this.ctx.globalAlpha = column.global_alpha;
        this.ctx.fillRect(
          column.left,
          column.draw_start_y,
          width,
          column.wall_height
        );
        this.ctx.globalAlpha = 1;
      }
    }
    return columns.map((c) => c.perp_wall_dist);
  }

  // draws columns left to right
  drawWallsRaycast(player: Player, map: GridMap): { [key: number]: number } {
    let ZBuffer: { [key: number]: number } = {};
    let width = Math.ceil(this.widthSpacing);

    // wall casting
    for (let column = 0; column < this.widthResolution; column++) {
      // x-coordinate in camera space scaled from -1 to 1
      let cameraX = (2 * column) / this.widthResolution - 1;

      // get the ray direction vector
      let rayDirX = player.position.dirX + player.position.planeX * cameraX;
      let rayDirY = player.position.dirY + player.position.planeY * cameraX;

      // which box of the map we're in
      let mapX = Math.floor(player.position.x);
      let mapY = Math.floor(player.position.y);

      // length of ray from current position to next x or y-side
      let sideDistX: number;
      let sideDistY: number;

      //length of ray from one x or y-side to next x or y-side
      //these are derived as:
      //deltaDistX = sqrt(1 + (rayDirY * rayDirY) / (rayDirX * rayDirX))
      //deltaDistY = sqrt(1 + (rayDirX * rayDirX) / (rayDirY * rayDirY))
      //which can be simplified to abs(|rayDir| / rayDirX) and abs(|rayDir| / rayDirY)
      //where |rayDir| is the length of the vector (rayDirX, rayDirY). Its length,
      //unlike (dirX, dirY) is not 1, however this does not matter, only the
      //ratio between deltaDistX and deltaDistY matters, due to the way the DDA
      //stepping further below works. So the values can be computed as below.
      // Division through zero is prevented
      let deltaDistX = Math.abs(1 / rayDirX);
      let deltaDistY = Math.abs(1 / rayDirY);
      // let deltaDistX =
      //   rayDirX === 0 ? 1e30 : Math.sqrt(1 + rayDirY ** 2 / rayDirX ** 2);
      // let deltaDistY =
      //   rayDirY === 0 ? 1e30 : Math.sqrt(1 + rayDirX ** 2 / rayDirY ** 2);

      // perpendicular wall distance
      let perpWallDist: number;

      // what direction to step in x or y-direction (either +1 or -1)
      let stepX: number;
      let stepY: number;

      let hit: number = 0; // was there a wall hit?
      let side: number; // was a NS or a EW wall hit? if x then side = 0, if y then side = 1

      // calculate step and initial sideDist
      if (rayDirX < 0) {
        stepX = -1;
        sideDistX = (player.position.x - mapX) * deltaDistX;
      } else {
        stepX = 1;
        sideDistX = (mapX + 1.0 - player.position.x) * deltaDistX;
      }

      if (rayDirY < 0) {
        stepY = -1;
        sideDistY = (player.position.y - mapY) * deltaDistY;
      } else {
        stepY = 1;
        sideDistY = (mapY + 1.0 - player.position.y) * deltaDistY;
      }

      // perform DDA
      let range = this.range;
      while (hit == 0 && range >= 0) {
        // jump to next map square, either in x-direction, or in y-direction
        if (sideDistX < sideDistY) {
          sideDistX += deltaDistX;
          mapX += stepX;
          side = 0;
        } else {
          sideDistY += deltaDistY;
          mapY += stepY;
          side = 1;
        }
        // Check if ray has hit a wall
        if (map.get(mapX, mapY) == 1) hit = 1;
        range -= 1;
      }
      // Calculate distance projected on camera direction. This is the shortest distance from the point where the wall is
      // hit to the camera plane. Euclidean to center camera plane would give fisheye effect!
      // This can be computed as (mapX - posX + (1 - stepX) / 2) / rayDirX for side == 0, or same formula with Y
      // for size == 1, but can be simplified to the code below thanks to how sideDist and deltaDist are computed:
      // because they were left scaled to |rayDir|. sideDist is the entire length of the ray above after the multiple
      // steps, but we subtract deltaDist once because one step more into the wall was taken above.
      if (side == 0) perpWallDist = sideDistX - deltaDistX;
      else perpWallDist = sideDistY - deltaDistY;
      // if (side === 0)
      //   perpWallDist = (mapX - player.position.x + (1 - stepX) / 2) / rayDirX;
      // else
      //   perpWallDist = (mapY - player.position.y + (1 - stepY) / 2) / rayDirY;

      // SET THE ZBUFFER FOR THE SPRITE CASTING
      ZBuffer[column] = perpWallDist; //perpendicular distance is used

      // Calculate height of line to draw on screen
      let lineHeight: number =
        this.width / 2 / player.position.planeYInitial / perpWallDist;

      // calculate lowest and highest pixel to fill in current stripe
      let drawStartY =
        -lineHeight / 2 +
        this.height / 2 +
        player.position.pitch +
        player.position.z;
      let drawEndY =
        lineHeight / 2 +
        this.height / 2 +
        player.position.pitch +
        player.position.z;

      let texture = map.wallTexture;

      // calculate value of wallX
      let wallX: number; // where exactly the wall was hit
      if (side == 0) wallX = player.position.y + perpWallDist * rayDirY;
      else wallX = player.position.x + perpWallDist * rayDirX;
      wallX -= Math.floor(wallX);

      // x coordinate on the texture
      let texX = Math.floor(wallX * texture.width);
      if (side == 0 && rayDirX > 0) texX = texture.width - texX - 1;
      if (side == 1 && rayDirY < 0) texX = texture.width - texX - 1;

      this.ctx.globalAlpha = 1;
      if (hit) {
        let left = Math.floor(column * this.widthSpacing);
        let wallHeight = drawEndY - drawStartY;

        this.ctx.drawImage(
          texture.image,
          texX, // sx
          0, // sy
          1, // sw
          texture.height, // sh
          left, // dx
          drawStartY, // dy - yes we go into minus here, it'll be ignored anyway
          width, // dw
          wallHeight // dh
        );

        // this is the shading of the texture - a sort of black overlay
        this.ctx.fillStyle = `#000000`;
        let alpha =
          (perpWallDist +
            // step.shading
            0) /
            this.lightRange -
          map.light;
        alpha = Math.min(alpha, 0.8);
        if (side == 1) {
          // give x and y sides different brightness
          alpha = alpha * 2;
        }
        alpha = Math.min(alpha, 0.85);
        // ensure walls are always at least a little bit visible - alpha 1 is all black
        this.ctx.globalAlpha = alpha;
        this.ctx.fillRect(left, drawStartY, width, wallHeight);
        this.ctx.globalAlpha = 1;
      }
    }

    return ZBuffer;
  }

  // draws columns left to right
  async drawSprites(
    sprites: Sprite[],
    player: Player,
    map: GridMap,
    ZBuffer: { [key: number]: number }
  ) {
    // SPRITE CASTING
    // sort sprites from far to close
    const sortedSprites = sortBy(
      sprites,
      (sprite) =>
        (player.position.x - sprite.x) * (player.position.x - sprite.x) +
        (player.position.y - sprite.y) * (player.position.y - sprite.y)
    ).reverse();

    // after sorting the sprites, do the projection and draw them
    for (let i = 0; i < sprites.length; i++) {
      // // translate sprite position to relative to camera
      let sprite = sortedSprites[i];

      let texture: Bitmap;
      let spriteTextureHeight = 1;
      switch (sprite.type) {
        case SpriteType.TREE_CONE:
          texture = map.treeTexture;
          spriteTextureHeight = 1.2;
          break;
        case SpriteType.TREE_VASE:
          texture = map.treeTextureVase;
          spriteTextureHeight = 0.3;
          break;
        case SpriteType.TREE_COLUMNAR:
          texture = map.treeTextureColumnar;
          spriteTextureHeight = 1.3;
          break;
        case SpriteType.PILLAR:
          texture = map.pillarTexture;
          spriteTextureHeight = 0.8;
          break;
        case SpriteType.BUSH1:
          texture = map.bush1Texture;
          spriteTextureHeight = 1;
          break;
      }

      const { screenX, screenYCeiling, screenYFloor, distance, fullHeight } =
        this.translateCoordinateToCamera(
          player,
          [sprite.x, sprite.y],
          spriteTextureHeight,
          false
        );

      // calculate width of the sprite
      let spriteWidth = Math.abs(
        // Math.floor(spriteTextureHeight * (this.width / 2 / distance))
        Math.floor(fullHeight * (texture.width / texture.height))
      );
      let drawStartX = Math.floor(-spriteWidth / 2 + screenX);
      if (drawStartX < 0) drawStartX = 0;
      let drawEndX = spriteWidth / 2 + screenX;
      if (drawEndX >= this.width) drawEndX = this.width - 1;

      const alpha = distance / this.lightRange - map.light;
      // ensure walls are always at least a little bit visible - alpha 1 is all black
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
          distance < ZBuffer[Math.floor(stripe / this.widthSpacing)]
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
        let texX1 = Math.floor(
          ((stripeParts[stripeIdx] - (-spriteWidth / 2 + screenX)) *
            texture.width) /
            spriteWidth
        );
        let texX2 = Math.ceil(
          ((stripeParts[stripeIdx + 1] - (-spriteWidth / 2 + screenX)) *
            texture.width) /
            spriteWidth
        );

        this.ctx.drawImage(
          texture.image,
          texX1, // sx
          0, // sy
          texX2 - texX1, // sw
          texture.height, // sh
          stripeParts[stripeIdx], // dx
          screenYCeiling, // dy
          stripeParts[stripeIdx + 1] - stripeParts[stripeIdx], // dw
          screenYFloor - screenYCeiling // dh
        );
      }
    }
    this.ctx.filter = `brightness(100%)`;
  }

  // draws columns left to right
  async drawColumns(player: Player, map: GridMap, spriteMap: SpriteMap) {
    this.ctx.save();

    // const { coords, sprites } = this.raycastVisibleCoordinates(
    //   spriteMap,
    //   player,
    //   map
    // );
    const { coords, sprites } = this.raycastVisibleCoordinatesWasm(
      spriteMap,
      player,
      map
    );
    // this.drawCeilingFloorTexture(coords, player, map);
    // this.drawCeilingFloorNoTexture(coords, player, map);
    // await this.drawCeilingFloorRaycast(player, map);
    await this.drawCeilingFloorRaycastWasm(player, map);

    let ZBuffer = this.drawWallsRaycastWasm(player, map);
    // let ZBuffer = this.drawWallsRaycast(player, map);

    // this.drawWallQuad(coords, player, map);
    this.drawSprites(sprites, player, map, ZBuffer);

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
