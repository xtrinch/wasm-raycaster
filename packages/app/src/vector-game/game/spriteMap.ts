import { makeAutoObservable } from "mobx";
import { level } from "./level";

export class SpriteMap {
  public size: number;
  public sprites: number[][];

  constructor() {
    // x, y, angle (0-360), height (multiplier of 1 z), type; placed in level.ts
    this.sprites = level.sprites;
    this.size = this.sprites.length;
    makeAutoObservable(this);
  }
}
