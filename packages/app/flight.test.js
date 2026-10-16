import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";

function load(file, wasm) {
  const source = readFileSync(new URL(`./src/vector-game/game/${file}.ts`, import.meta.url), "utf8");
  const js = ts.transpileModule(source, { compilerOptions: {
    target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS,
  } }).outputText;
  const exports = {};
  runInNewContext(`(function(require, exports) { ${js}\n})`, {
    document: { addEventListener() {} },
  })(name => {
    if (name === "mobx") return { makeAutoObservable() {} };
    if (name === "../../../wasm") return wasm;
    if (name === "./bitmap") return { Bitmap: class {} };
    if (name.endsWith(".png")) return { default: "" };
    throw new Error(`Unexpected import: ${name}`);
  }, exports);
  return exports;
}

const { Controls } = load("controls");
const controls = () => ({ ...new Controls().states });
function fixture(x = 0, z = 0) {
  const map = { size: 3, wallTexture: { width: 32 } };
  const { Player } = load("player", {
    // A roofed room occupies x=1..2. Rust tests check real grid coverage.
    roof_height: x => x >= 1 && x < 2 ? 1 : 0,
    walk: (x, y, dx, dy, _px, _py, _pitch, _z, _initial, distance) => {
      const next = x + dx * distance;
      return next >= 1 && next < 2 && x < 1 ? [x, y] : [next, y + dy * distance];
    },
  });
  const camera = { height: 600, width: 800, range: 40, mapRef: { ptr: 0 }, map };
  const player = new Player(x, 0.5, z, 1, 0, 0, 1.1, camera);
  const tick = keys => player.update({ ...controls(), ...keys }, map, 0.1);
  return { player, map, tick };
}

test("G/H control flight and leave D/F bindings intact", () => {
  const c = new Controls();
  for (const [keyCode, state] of [[71, "jetpackUp"], [72, "jetpackDown"], [68, "jumpUp"], [70, "jumpDown"]]) {
    c.onKey({ keyCode, type: "keydown" });
    assert.equal(c.states[state], true);
    c.onKey({ keyCode, type: "keyup" });
    assert.equal(c.states[state], false);
  }
});

test("ascend, hover, descend and land on ground", () => {
  const { player, tick } = fixture();
  for (let i = 0; i < 15; i++) tick({ jetpackUp: true });
  assert.equal(player.position.z, 900);
  tick({});
  assert.equal(player.position.z, 900);
  for (let i = 0; i < 15; i++) tick({ jetpackDown: true });
  assert.equal(player.position.z, 0);
  assert.equal(player.jetpackActive, false);
});

test("fly over room, land on roof, walk and fall off edge", () => {
  const { player, map, tick } = fixture();
  for (let i = 0; i < 15; i++) tick({ jetpackUp: true });
  player.walk(1.5, map);
  for (let i = 0; i < 6; i++) tick({ jetpackDown: true });
  assert.equal(player.position.z, 600);
  assert.equal(player.jetpackActive, false);
  tick({});
  assert.equal(player.position.z, 600);
  player.jumpDown(0.1);
  assert.equal(player.position.z, 600);
  player.walk(0.7, map);
  tick({});
  assert.equal(player.position.z, 590);
});

test("ceilings block ascent and low flight still uses wall collision", () => {
  const inside = fixture(1.5);
  for (let i = 0; i < 20; i++) inside.tick({ jetpackUp: true });
  assert.equal(inside.player.position.z, 294);
  const outside = fixture(0.9);
  outside.tick({ jetpackUp: true });
  outside.player.walk(0.3, outside.map);
  assert.ok(outside.player.position.x < 1);
});
