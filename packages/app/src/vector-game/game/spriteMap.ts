import { makeAutoObservable } from "mobx";

export enum SpriteType {
  TREE_CONE = 1,
  PILLAR = 2,
  BUSH1 = 3,
  TREE_VASE = 4,
  TREE_COLUMNAR = 5,
  LADY = 6,
}

export class SpriteMap {
  public size: number;
  public sprites: number[][];

  constructor() {
    // x, y, angle (0-360), height (multiplier of 1 z), type
    this.sprites = [
      [-1, 5, 0, 1.5, SpriteType.TREE_CONE],
      [-2, 5, 0, 0.5, SpriteType.TREE_CONE],
      [-1, 6, 0, 0.9, SpriteType.BUSH1],
      [-2, 4, 0, 1, SpriteType.TREE_VASE],
      [2.3, 3.3, 90, 0.8, SpriteType.LADY],
      [0.8, 3.2, 270, 0.8, SpriteType.LADY],
      [4, 7, 0, 0.7, SpriteType.PILLAR],
      [14, 8.5, 0, 0.5, SpriteType.PILLAR],
      [-0.5, 1.5, 0, 1, SpriteType.TREE_CONE],
      [-0.5, 3.5, 0, 1, SpriteType.TREE_COLUMNAR],
      [18.5, 4.5, 0, 1, SpriteType.TREE_CONE],
      [12.5, 5, 0, 1, SpriteType.TREE_VASE],
      [12.5, 4.5, 0, 1, SpriteType.TREE_CONE],
      [12.5, 12.5, 0, 1, SpriteType.TREE_CONE],
      [3.5, 20.5, 0, 1, SpriteType.TREE_CONE],
      [3.5, 14.5, 0, 1, SpriteType.TREE_CONE],
      [14.5, 20.5, 0, 1, SpriteType.TREE_CONE],
      [18.5, 10.5, 0, 1, SpriteType.TREE_CONE],
      [18.5, 11.5, 0, 1, SpriteType.TREE_CONE],
      [18.5, 12.5, 0, 1, SpriteType.TREE_CONE],
      [21.5, 1.5, 0, 1, SpriteType.TREE_CONE],
      [15.5, 0.5, 0, 1, SpriteType.TREE_CONE],
      [16.0, 0.8, 0, 1, SpriteType.TREE_CONE],
      [16.2, 0.2, 0, 1, SpriteType.TREE_CONE],
      [9.5, 15.5, 0, 1, SpriteType.TREE_CONE],
      [10.0, 15.1, 0, 1, SpriteType.TREE_CONE],
      [10.5, 15.8, 0, 1, SpriteType.TREE_CONE],
    ];
    this.size = this.sprites.length;
    makeAutoObservable(this);
  }
}
