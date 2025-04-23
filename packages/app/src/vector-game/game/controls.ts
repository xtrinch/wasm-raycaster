import { makeAutoObservable } from "mobx";

export interface ControlStates {
  left: boolean;
  right: boolean;
  backward: boolean;
  forward: boolean;
  lookUp: boolean;
  lookDown: boolean;
  jumpUp: boolean;
  jumpDown: boolean;
}

export class Controls {
  public codes = {
    37: "left",
    39: "right",
    38: "forward",
    40: "backward",
    65: "lookUp", // a
    83: "lookDown", // s
    68: "jumpUp", // d
    70: "jumpDown", // f
  };
  public states: ControlStates = {
    left: false,
    right: false,
    forward: false,
    backward: false,
    lookUp: false,
    lookDown: false,
    jumpUp: false,
    jumpDown: false,
  };

  constructor() {
    // TODO; THROTTLE?
    document.addEventListener("keydown", this.onKey.bind(this), false);
    document.addEventListener("keyup", this.onKey.bind(this), false);

    makeAutoObservable(this);
  }

  onKey(e: KeyboardEvent) {
    let state: string = this.codes[e.keyCode];
    if (typeof state === "undefined") return;
    this.states[state as keyof ControlStates] =
      e.type === "keyup" ? false : true;
    // e.preventDefault && e.preventDefault();
    // e.stopPropagation && e.stopPropagation();
  }
}
