import { makeAutoObservable } from "mobx";

export class Bitmap {
  public image: HTMLImageElement;
  public width: number;
  public height: number;
  public filename: string;

  constructor(src: string, width: number, height: number) {
    this.image = new Image();
    this.image.src = src;
    this.width = width;
    this.height = height;
    this.filename = src;

    makeAutoObservable(this);
  }
}
