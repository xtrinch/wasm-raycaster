import { makeAutoObservable } from "mobx";
import { move_vertical, rotate_view, walk } from "../../../wasm";
import knifeHand from "../../assets/knife_hand.png";
import { Bitmap } from "./bitmap";
import { Camera } from "./camera";
import { ControlStates } from "./controls";
import { GridMap } from "./gridMap";
export interface Position {
  x: number; // pos x of player
  y: number; // pos y of player
  z: number; // pos z of player
  dir_x: number; // x component of direction vector
  dir_y: number; // y component of direction vector
  plane_x: number; // x component of camera plane
  plane_y: number; // y component of camera plane
  pitch: number;
  plane_y_initial: number;
}

export class Player {
  public weapon: Bitmap;
  public paces: number;
  public position: Position;
  public camera: Camera;
  // Stay hovering after G/H is released; landing switches gravity back on.
  public jetpackActive = false;

  constructor(
    x: number,
    y: number,
    z: number,
    dir_x: number,
    dir_y: number,
    plane_x: number,
    plane_y: number,
    camera: Camera
  ) {
    this.position = {
      x,
      y,
      z,
      dir_x,
      dir_y,
      plane_x,
      plane_y,
      pitch: 0,
      plane_y_initial:
        Math.abs(plane_y) > Math.abs(plane_x)
          ? Math.abs(plane_y)
          : Math.abs(plane_x), // basically direction vector length; TODO
    };
    this.weapon = new Bitmap(knifeHand, 319, 320);
    this.paces = 0;
    this.camera = camera;

    makeAutoObservable(this);
  }

  public rotate = (frameTime: number, multiplier: number) => {
    const [dir_x, dir_y, plane_x, plane_y] = rotate_view(
      frameTime,
      multiplier,
      this.position.dir_x,
      this.position.dir_y,
      this.position.plane_x,
      this.position.plane_y
    );
    this.position.dir_x = dir_x;
    this.position.dir_y = dir_y;
    this.position.plane_x = plane_x;
    this.position.plane_y = plane_y;
  };

  // move if no wall in front of you
  public walk = (distance: number, map: GridMap) => {
    const [x, y] = walk(
      this.position.x,
      this.position.y,
      this.position.dir_x,
      this.position.dir_y,
      this.position.plane_x,
      this.position.plane_y,
      this.position.pitch,
      this.position.z,
      this.position.plane_y_initial,
      distance,
      this.camera.mapRef.ptr,
      map.size,
      this.camera.width,
      this.camera.height,
      this.camera.range,
      map.wallTexture.width
    );
    this.position.x = x;
    this.position.y = y;
  };

  public jumpUp = (frameTime: number) => this.moveVertical(1, frameTime);

  public jumpDown = (frameTime: number) => this.moveVertical(-1, frameTime);

  private moveVertical = (direction: number, frameTime: number) => {
    const [z, landed] = move_vertical(
      this.position.x,
      this.position.y,
      this.position.z,
      this.camera.height,
      frameTime,
      direction,
      this.jetpackActive,
      this.camera.mapRef.ptr,
      this.camera.map.size
    );
    this.position.z = z;
    // Rust reports landing; stop hovering so walking off an edge causes a fall.
    if (landed) this.jetpackActive = false;
  };

  public lookDown = (frameTime: number) => {
    // look down
    this.position.pitch -= Math.floor(400 * frameTime);
    if (this.position.pitch < -200) this.position.pitch = -200;
  };

  public lookUp = (frameTime: number) => {
    // look up
    this.position.pitch += 400 * frameTime;
    if (this.position.pitch > 200) this.position.pitch = 200;
  };

  private updateHeight = (controls: ControlStates, frameTime: number) => {
    if (controls.jetpackUp || controls.jetpackDown) {
      this.jetpackActive = true;
      // Opposite flight keys cancel; releasing both keeps hovering active.
      const direction = Number(controls.jetpackUp) - Number(controls.jetpackDown);
      this.moveVertical(direction, frameTime);
    } else if (!this.jetpackActive && controls.jumpDown) {
      this.jumpDown(frameTime);
    } else if (!this.jetpackActive && controls.jumpUp) {
      this.jumpUp(frameTime);
    } else {
      this.moveVertical(0, frameTime);
    }
  };

  public update = (
    controls: ControlStates,
    map: GridMap,
    frameTime: number
  ) => {
    if (controls.left) this.rotate(frameTime, 1);
    else if (controls.right) this.rotate(frameTime, -1);

    if (controls.forward) this.walk(3 * frameTime, map);
    else if (controls.backward) this.walk(-3 * frameTime, map);

    this.updateHeight(controls, frameTime);

    if (controls.lookDown) this.lookDown(frameTime);
    else if (controls.lookUp) this.lookUp(frameTime);

    if (this.position.pitch > 0)
      this.position.pitch = Math.floor(
        Math.max(0, this.position.pitch - 100 * frameTime)
      );
    else if (this.position.pitch < 0)
      this.position.pitch = Math.floor(
        Math.min(0, this.position.pitch + 100 * frameTime)
      );

  };
}
