import { makeAutoObservable } from "mobx";
import { Position as RustPosition, walk } from "../../../wasm";
import knifeHand from "../../assets/knife_hand.png";
import { Bitmap } from "./bitmap";
import { Camera } from "./camera";
import { ControlStates } from "./controls";
import { GridMap } from "./gridMap";
export interface Position {
  x: number; // pos x of player
  y: number; // pos y of player
  z: number; // pos z of player
  dirX: number; // x component of direction vector
  dirY: number; // y component of direction vector
  planeX: number; // x component of camera plane
  planeY: number; // y component of camera plane
  pitch: number;
  planeYInitial: number;
}

export class Player {
  public weapon: Bitmap;
  public paces: number;
  public position: Position;
  public camera: Camera;

  constructor(
    x: number,
    y: number,
    z: number,
    dirX: number,
    dirY: number,
    planeX: number,
    planeY: number,
    camera: Camera
  ) {
    this.position = {
      x,
      y,
      z,
      dirX,
      dirY,
      planeX,
      planeY,
      pitch: 0,
      planeYInitial:
        Math.abs(planeY) > Math.abs(planeX)
          ? Math.abs(planeY)
          : Math.abs(planeX), // basically direction vector length; TODO
    };
    this.weapon = new Bitmap(knifeHand, 319, 320);
    this.paces = 0;
    this.camera = camera;

    makeAutoObservable(this);
  }

  public rotate = (angle: number) => {
    const rotSpeed = angle;

    let oldDirX = this.position.dirX;
    this.position.dirX =
      this.position.dirX * Math.cos(-rotSpeed) -
      this.position.dirY * Math.sin(-rotSpeed);
    this.position.dirY =
      oldDirX * Math.sin(-rotSpeed) + this.position.dirY * Math.cos(-rotSpeed);

    let oldPlaneX = this.position.planeX;
    this.position.planeX =
      this.position.planeX * Math.cos(-rotSpeed) -
      this.position.planeY * Math.sin(-rotSpeed);
    this.position.planeY =
      oldPlaneX * Math.sin(-rotSpeed) +
      this.position.planeY * Math.cos(-rotSpeed);

    console.log(
      this.position.dirX,
      this.position.dirY,
      this.position.planeX,
      this.position.planeY
    );
  };

  // move if no wall in front of you
  public walk = (distance: number, map: GridMap) => {
    const [x, y] = walk(
      this.toRustPosition(),
      distance,
      this.camera.mapRef.ptr,
      map.size,
      this.camera.widthResolution,
      this.camera.width,
      this.camera.height,
      this.camera.widthSpacing,
      this.camera.lightRange,
      this.camera.range,
      map.wallTexture.width
    );
    console.log(x, y);
    this.position.x = x;
    this.position.y = y;
  };

  public jumpUp = (frameTime: number) => {
    this.position.z += 400 * frameTime;
    if (this.position.z > 300) this.position.z = 300;
    // if (this.position.z > 9000) this.position.z = 9000;
  };

  public jumpDown = (frameTime: number) => {
    this.position.z -= 400 * frameTime;
    if (this.position.z < 0) this.position.z = 0;
  };

  public lookDown = (frameTime: number) => {
    // look down
    this.position.pitch -= 400 * frameTime;
    if (this.position.pitch < -200) this.position.pitch = -200;
  };

  public lookUp = (frameTime: number) => {
    // look up
    this.position.pitch += 400 * frameTime;
    if (this.position.pitch > 200) this.position.pitch = 200;
  };

  public update = (
    controls: ControlStates,
    map: GridMap,
    frameTime: number
  ) => {
    if (controls.left) this.rotate(4 * (-Math.PI / 5) * frameTime);
    if (controls.right) this.rotate(4 * (Math.PI / 5) * frameTime);
    if (controls.forward) this.walk(3 * frameTime, map);
    if (controls.backward) this.walk(-3 * frameTime, map);
    if (controls.jumpDown) this.jumpDown(frameTime);
    if (controls.jumpUp) this.jumpUp(frameTime);
    if (controls.lookDown) this.lookDown(frameTime);
    if (controls.lookUp) this.lookUp(frameTime);

    if (this.position.pitch > 0)
      this.position.pitch = Math.max(0, this.position.pitch - 100 * frameTime);
    if (this.position.pitch < 0)
      this.position.pitch = Math.min(0, this.position.pitch + 100 * frameTime);
    if (this.position.z > 0)
      this.position.z = Math.max(0, this.position.z - 100 * frameTime);
  };

  toRustPosition(): RustPosition {
    return {
      x: this.position.x,
      y: this.position.y,
      dir_x: this.position.dirX,
      dir_y: this.position.dirY,
      plane_x: this.position.planeX,
      plane_y: this.position.planeY,
      z: this.position.z,
      plane_y_initial: this.position.planeYInitial,
      pitch: this.position.pitch,
    } as RustPosition;
  }
}
