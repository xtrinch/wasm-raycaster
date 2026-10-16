import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const source = readFileSync(new URL("./src/vector-game/game/elevation.ts", import.meta.url), "utf8");
const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.ESNext } }).outputText;
const { stairCell, buildStairBoxes, gridSupportHeight, stairHeightAt, STEP_HEIGHT, STAIR_START_Y, STAIR_END_Y } =
  await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
// Check the real packed map as well as synthetic geometry.
const mapSource = readFileSync(new URL("./src/vector-game/game/gridMap.ts", import.meta.url), "utf8");
const packed = mapSource.match(/this\.wallGrid = new BigUint64Array\(\[([\s\S]*?)\]\.map/)[1];
const declaredValues = runInNewContext(`[${packed}]`, { stairCell });
const declaredGrid = new BigUint64Array(declaredValues.map(v => BigInt(v)));
const size = Math.sqrt(declaredGrid.length);
const grid = declaredGrid.slice();
// Bare building fixture; production layout is checked separately below.
for (let i=0;i<grid.length;i++) if (grid[i] & (1n << 10n)) grid[i]=8n;
grid[5]=8n;
grid[6]=8n;

const elevation = await import(`data:text/javascript;base64,${Buffer.from(js).toString("base64")}`);
function mapWithStairs(x) {
  const map = grid.slice();
  map[x] = stairCell("south","lower");
  map[size+x] = stairCell("south","upper");
  return map;
}

test("packed grid alone reconstructs stairs after a buffer round trip", () => {
  for (let x = 3; x <= 9; x++) {
    const map = mapWithStairs(x);
    const restored = new BigUint64Array(map.buffer.slice(0));
    const { stairs } = buildStairBoxes(restored,size);
    assert.equal(stairs.length,12);
    assert.equal(stairs[0][0],x);
    for (const step of stairs) {
      const y = (step[1]+step[3])/2;
      assert.equal(stairHeightAt(restored,size,x+0.45,y),step[5]);
      assert.equal(gridSupportHeight(restored,size,x+0.45,y,step[5]),step[5]);
    }
    for (const row of [0,1]) {
      assert.ok(restored[row*size+x] & elevation.STAIR_CELL_BIT);
      assert.equal((restored[row*size+x] >> 12n) & 15n,0n, "stair metadata is not a wall segment");
      assert.equal(restored[row*size+x] & 8n,8n, "existing ground material is preserved");
      assert.equal(restored[row*size+x] >> 19n,0n,"no dimensions are stored in a stair cell");
    }
  }
});

test("removing a stair grid cell removes both rendered geometry and support there", () => {
  const map = mapWithStairs(4);
  map[4] = grid[4];
  assert.equal(stairHeightAt(map,size,4.45,0.5),undefined);
  assert.equal(gridSupportHeight(map,size,4.45,0.5,0),0);
  assert.ok(stairHeightAt(map,size,4.45,1.5) > 0);
  const { stairs } = buildStairBoxes(map,size);
  assert.ok(stairs.length > 0);
  assert.ok(stairs.every(box => box[1] >= 1), "no stale off-grid tread remains");
});

test("map literal declares directional stairs and shed in their actual cells", () => {
  assert.equal(declaredGrid[4],stairCell("south","lower"));
  assert.equal(declaredGrid[size+4],stairCell("south","upper"));
  assert.equal(elevation.stairDescriptor(declaredGrid[4]).facing,"south");
  assert.equal(elevation.stairDescriptor(declaredGrid[4]).section,"lower");
  assert.equal(elevation.stairDescriptor(declaredGrid[size+4]).section,"upper");
  assert.equal(buildStairBoxes(declaredGrid,size).stairs.length,12);
  assert.equal((declaredGrid[5] >> 12n) & 3n,3n);
  assert.equal((declaredGrid[6] >> 12n) & 3n,3n);
  assert.equal(buildStairBoxes(grid,size).stairs.length,0);
});

test("stair entries accept only a direction and lower/upper section", () => {
  assert.throws(() => stairCell("up","lower"),/Invalid stair direction/);
  assert.throws(() => stairCell("south","middle"),/Invalid stair direction/);
  assert.throws(() => stairCell("south","single"),/Invalid stair direction/);
  assert.throws(() => stairCell("south"),/Invalid stair direction/);
});

for (const [direction,first,second] of [
  ["north",[1,2],[1,1]], ["east",[0,1],[1,1]],
  ["south",[1,0],[1,1]], ["west",[2,1],[1,1]],
]) {
  test(`${direction} flight continues across its declared neighboring cell`, () => {
    const map = new BigUint64Array(9);
    map[first[1]*3+first[0]] = stairCell(direction,"lower");
    map[second[1]*3+second[0]] = stairCell(direction,"upper");
    const { stairs } = buildStairBoxes(map,3);
    assert.equal(stairs.length,12,"split tread is rejoined in either axis");
    for (const box of stairs) {
      assert.equal(stairHeightAt(map,3,(box[0]+box[2])/2,(box[1]+box[3])/2),box[5]);
    }
  });
}

test("shed walls form a flush rectangle matching the roof footprint", () => {
  const map = declaredGrid;
  const rectangles = [];
  for (let x = 5; x <= 6; x++) {
    const cell = map[x];
    for (let i = 0; i < 3; i++) {
      const bits = cell >> BigInt(16 + 16 * i);
      const primary = Number(bits & 15n) / 10;
      const thickness = Number((bits >> 4n) & 15n) / 10;
      const length = Number((bits >> 8n) & 15n) / 10;
      const secondary = Number((bits >> 12n) & 15n) / 10;
      const north = Boolean(cell & (1n << BigInt([6,7,2][i])));
      rectangles.push(north
        ? [x+secondary,primary,x+secondary+length,primary+thickness]
        : [x+primary,secondary,x+primary+thickness,secondary+length]);
    }
  }
  assert.deepEqual(rectangles, [
    [5,0,5.1,1], [5,0,6,0.1], [5,0.9,6,1],
    [6.9,0,7,1], [6,0,7,0.1], [6,0.9,7,1],
  ]);
  assert.equal(gridSupportHeight(map,size,5.5,0.5,1),1);
  assert.equal(gridSupportHeight(map,size,6.5,0.5,1),1);
});
const playerSource = readFileSync(new URL("./src/vector-game/game/player.ts", import.meta.url), "utf8");
const playerJs = ts.transpileModule(playerSource, { compilerOptions: { target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS } }).outputText;
const playerExports = {};
runInNewContext(`(function(require, exports) { ${playerJs}\n})`)(name => {
  if (name === "mobx") return { makeAutoObservable() {} };
  if (name === "../../../wasm") return {
    roof_height: () => 0,
    // Existing ground collision is Rust-owned; simulate unobstructed ground here.
    walk: (x,y,dx,dy,_px,_py,_pitch,_z,_initial,distance) => [x + dx * distance, y + dy * distance],
  };
  if (name === "./elevation") return elevation;
  if (name === "./bitmap") return { Bitmap: class {} };
  if (name.endsWith(".png")) return { default: "" };
  throw new Error(`Unexpected runtime import ${name}`);
}, playerExports);

function playerAt(x, y, dx = 0, dy = 1) {
  const camera = { height: 600, width: 800, range: 40, mapRef: { ptr: 0 } };
  const world = mapWithStairs(4);
  const { stairs } = buildStairBoxes(world, size);
  const map = { stairs, size, wallTexture: { width: 1024 },
    supportHeight: (x,y,feet) => gridSupportHeight(world,size,x,y,feet),
    stairHeight: (x,y) => stairHeightAt(world,size,x,y) };
  camera.map = map;
  return { player: new playerExports.Player(x,y,0,dx,dy,1.1,0,camera), map };
}
const idle = { left:false,right:false,forward:false,backward:false,jumpDown:false,jumpUp:false,lookDown:false,lookUp:false };

for (const [direction,x,y,dx,dy,lower,upper] of [
  ["north",1.45,3.05,0,-1,7,4], ["east",-0.05,1.45,1,0,3,4],
  ["south",1.45,-0.05,0,1,1,4], ["west",3.05,1.45,-1,0,5,4],
]) {
  test(`Player ascends and descends ${direction}-facing lower/upper stairs`, () => {
    const world = new BigUint64Array(9);
    world[lower] = stairCell(direction,"lower");
    world[upper] = stairCell(direction,"upper");
    const map = { size:3,wallTexture:{width:32},
      supportHeight:(x,y,feet) => gridSupportHeight(world,3,x,y,feet),
      stairHeight:(x,y) => stairHeightAt(world,3,x,y) };
    const camera = { height:600,width:800,range:40,mapRef:{ptr:0} };
    camera.map = map;
    const player = new playerExports.Player(x,y,0,dx,dy,dy*1.1,-dx*1.1,camera);
    player.walk(2,map);
    assert.equal(player.position.z,600);
    player.walk(-2,map);
    assert.equal(player.position.z,0);
  });
}

test("south-facing stair fixtures meet the real building roof at wall-top height", () => {
  for (let x = 3; x <= 9; x++) {
    const world = mapWithStairs(x);
    const { stairs } = buildStairBoxes(world, size);
    assert.equal(stairs.length, 12);
    assert.equal(stairs.at(-1)[5], 1);
    // Decode the real north wall's outer face; the stairs must not overlap it.
    const northWall = grid[2 * size + x];
    const outerY = 2 + Number((northWall >> 16n) & 15n) / 10;
    assert.ok(stairs.at(-1)[3] <= outerY, "stairs stay outside the interior wall");
    assert.equal(gridSupportHeight(world, size, x + 0.45, STAIR_END_Y + 0.01, 1), 1);
    for (let i = 1; i < stairs.length; i++) {
      assert.equal(stairs[i - 1][3], stairs[i][1]);
      assert.ok(stairs[i][5] - stairs[i - 1][5] <= STEP_HEIGHT + 1e-9);
    }
  }
});

test("roof support comes from grid cells without generating roof boxes", () => {
  assert.equal(Object.hasOwn(buildStairBoxes(grid,size),"roofs"),false);
  for (let y = 0; y < size; y++) for (let x = 0; x < size; x++) {
    assert.equal(gridSupportHeight(grid, size, x + 0.5, y + 0.5, 1), grid[y * size + x] & 2n ? 1 : 0);
    assert.equal(gridSupportHeight(grid, size, x + 0.5, y + 0.5, 0), 0, "ground-floor player stays under roof");
  }
});

test("walk up all treads, reverse down them, and leave the foot on ground", () => {
  const world = mapWithStairs(4);
  const { stairs } = buildStairBoxes(world, size);
  let feet = 0;
  for (let y = STAIR_START_Y; y < STAIR_END_Y; y += 0.01) {
    const next = gridSupportHeight(world, size, 4.45, y, feet);
    assert.ok(next - feet <= STEP_HEIGHT + 1e-9);
    feet = next;
  }
  assert.equal(feet, 1);
  for (let y = STAIR_END_Y - 0.01; y > STAIR_START_Y; y -= 0.01) {
    const next = gridSupportHeight(world, size, 4.45, y, feet);
    assert.ok(feet - next <= STEP_HEIGHT + 1e-9);
    feet = next;
  }
  assert.equal(gridSupportHeight(world, size, 4.45, STAIR_START_Y - 0.01, feet), 0);
});

test("Player climbs onto roof, stays there, and descends without a teleport", () => {
  const { player, map } = playerAt(4.45,-1.2);
  player.walk(4.5,map); // Includes many substeps, even after a delayed frame.
  player.update(idle,map,0.016);
  assert.ok(player.position.y > 3);
  assert.equal(player.position.z,600);
  player.update({ ...idle,jumpDown:true },map,1);
  assert.equal(player.position.z,600, "lowering view cannot push through roof");
  player.walk(-4.5,map);
  assert.equal(player.position.z,0, "descent follows each tread without floating above the stairs");
  for (let i = 0; i < 70; i++) player.update(idle,map,0.1);
  assert.equal(player.position.z,0);
  assert.ok(player.position.y < STAIR_START_Y);
});

test("Player cannot enter high stair sides from the ground", () => {
  const { player,map } = playerAt(3.95,1,1,0);
  player.walk(0.5,map);
  assert.ok(player.position.x < 4);
  assert.equal(player.position.z,0);
});

test("walking over a roof edge lowers the player back to ground", () => {
  const { player,map } = playerAt(10.9,3.5,1,0);
  player.position.z = 600;
  player.walk(2,map);
  for (let i = 0; i < 70; i++) player.update(idle,map,0.1);
  assert.equal(player.position.z,0);
});
