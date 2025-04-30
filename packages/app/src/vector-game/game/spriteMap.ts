import { makeAutoObservable } from "mobx";

export enum TextureType {
  WALL = 1,
  CEILING = 2,
  FLOOR = 3,
  ROAD = 4,
  DOOR = 5,
  TREE_CONE = 6,
  PILLAR = 7,
  BUSH1 = 8,
  TREE_VASE = 9,
  TREE_COLUMNAR = 10,
  LADY = 11,
  WINDOW = 12,
}

export class SpriteMap {
  public size: number;
  public sprites: number[][];

  constructor() {
    // x, y, angle (0-360), height (multiplier of 1 z), type
    this.sprites = [
      [3, -1, 0, 1, TextureType.TREE_COLUMNAR],
      [1, 7, 0, 1, TextureType.TREE_COLUMNAR],
      [-1, 5, 0, 1.5, TextureType.TREE_CONE],
      [-2, 5, 0, 0.5, TextureType.TREE_CONE],
      [-1, 6, 0, 0.9, TextureType.BUSH1],
      [-2, 4, 0, 1, TextureType.TREE_VASE],
      [2.4, 4.2, 90, 0.8, TextureType.LADY],
      [0.8, 3.2, 270, 0.8, TextureType.LADY],
      [4, 7, 0, 0.7, TextureType.PILLAR],
      [5, 5, 0, 0.7, TextureType.PILLAR],
      [14, 8.5, 0, 0.5, TextureType.PILLAR],
      [-0.5, 1.5, 0, 1, TextureType.TREE_CONE],
      [-0.5, 3.5, 0, 1, TextureType.TREE_COLUMNAR],
      [18.5, 4.5, 0, 1, TextureType.TREE_CONE],
      [12.5, 5, 0, 1, TextureType.TREE_VASE],
      [12.5, 4.5, 0, 1, TextureType.TREE_CONE],
      [12.5, 12.5, 0, 1, TextureType.TREE_CONE],
      [3.5, 20.5, 0, 1, TextureType.TREE_CONE],
      [3.5, 14.5, 0, 1, TextureType.TREE_CONE],
      [14.5, 20.5, 0, 1, TextureType.TREE_CONE],
      [18.5, 10.5, 0, 1, TextureType.TREE_CONE],
      [18.5, 11.5, 0, 1, TextureType.TREE_CONE],
      [18.5, 12.5, 0, 1, TextureType.TREE_CONE],
      [21.5, 1.5, 0, 1, TextureType.TREE_CONE],
      [15.5, 0.5, 0, 1, TextureType.TREE_CONE],
      [16.0, 0.8, 0, 1, TextureType.TREE_CONE],
      [16.2, 0.2, 0, 1, TextureType.TREE_CONE],
      [9.5, 15.5, 0, 1, TextureType.TREE_CONE],
      [10.0, 15.1, 0, 1, TextureType.TREE_CONE],
      [10.5, 15.8, 0, 1, TextureType.TREE_CONE],
    ];
    this.size = this.sprites.length;
    makeAutoObservable(this);
  }
}
