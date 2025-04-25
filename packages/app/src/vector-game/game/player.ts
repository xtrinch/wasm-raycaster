import { makeAutoObservable } from "mobx";
import { walk } from "../../../wasm";
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

  public rotate = (angle: number) => {
    const rotSpeed = angle;

    let olddir_x = this.position.dir_x;
    this.position.dir_x =
      this.position.dir_x * Math.cos(-rotSpeed) -
      this.position.dir_y * Math.sin(-rotSpeed);
    this.position.dir_y =
      olddir_x * Math.sin(-rotSpeed) +
      this.position.dir_y * Math.cos(-rotSpeed);

    let oldplane_x = this.position.plane_x;
    this.position.plane_x =
      this.position.plane_x * Math.cos(-rotSpeed) -
      this.position.plane_y * Math.sin(-rotSpeed);
    this.position.plane_y =
      oldplane_x * Math.sin(-rotSpeed) +
      this.position.plane_y * Math.cos(-rotSpeed);
  };

  // move if no wall in front of you
  public walk = (distance: number, map: GridMap) => {
    const [x, y] = walk(
      this.position,
      distance,
      this.camera.mapRef.ptr,
      map.size,
      this.camera.width,
      this.camera.width,
      this.camera.height,
      1,
      this.camera.lightRange,
      this.camera.range,
      map.wallTexture.width
    );
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
    this.position.pitch -= Math.floor(400 * frameTime);
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
      this.position.pitch = Math.floor(
        Math.max(0, this.position.pitch - 100 * frameTime)
      );
    else if (this.position.pitch < 0)
      this.position.pitch = Math.floor(
        Math.min(0, this.position.pitch + 100 * frameTime)
      );

    if (this.position.z > 0)
      this.position.z = Math.max(0, this.position.z - 100 * frameTime);
  };
}
