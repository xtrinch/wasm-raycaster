import { max, mean } from "lodash";
import { makeAutoObservable } from "mobx";
import { Camera } from "./camera";
import { Controls } from "./controls";
import { GridMap } from "./gridMap";
import { Player } from "./player";
import { SpriteMap } from "./spriteMap";

export class GameLoop {
  public lastTime = 0;
  public display: HTMLCanvasElement;
  public player: Player;
  public map: GridMap;
  public controls: Controls;
  public camera: Camera;
  public spriteMap: SpriteMap;
  public fps: number;
  public lastNFrameTimes: number[];
  public frameTime: number;

  constructor() {
    this.map = new GridMap(12);
    this.spriteMap = new SpriteMap();
    this.display = document.getElementById("display") as HTMLCanvasElement;
    this.controls = new Controls();
    this.camera = new Camera(this.display, this.map, this.spriteMap);
    this.player = this.findSpawnPoint();
    this.fps = 0;
    this.frameTime = 0;
    this.lastNFrameTimes = [];
    makeAutoObservable(this);
  }

  frame(time: number) {
    this.frameTime = (time - this.lastTime) / 1000;

    if (this.frameTime > 0.01) {
      this.lastTime = time;
      this.loop();
    }

    if (this.lastNFrameTimes.length >= 10) {
      this.lastNFrameTimes.shift();
    }
    this.lastNFrameTimes.push(this.frameTime);
    const avgFrameTime = this.getAverageFrameTime();
    const fps = Math.floor(1.0 / avgFrameTime);
    this.fps = fps;

    requestAnimationFrame(this.frame.bind(this));
  }

  start() {
    requestAnimationFrame(this.frame.bind(this));
  }

  getAverageFrameTime() {
    const avgFrameTime = mean(this.lastNFrameTimes);
    return avgFrameTime;
  }

  getMinFPS() {
    const minFrameTime = max(this.lastNFrameTimes);
    const minFps = Math.floor(1.0 / minFrameTime);

    return minFps;
  }

  loop() {
    // this.map.update(this.frameTime);
    this.player.update(this.controls.states, this.map, this.frameTime);
    this.camera.render(this.player);
    this.camera.drawCanvas();
  }

  findSpawnPoint() {
    // return new Player(x + 0.5, y + 0.5, -1, 0, 0, 0.66); // original
    // return new Player(4, 4, 0, -1, 0, 0, 1.1, this.camera); // looking east
    return new Player(4, 4, 0, 0, -1, -1.1, 0, this.camera); // looking north
  }
}
