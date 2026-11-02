// Encodes and decodes single GridMap.wallGrid cells in the u64 layout documented
// in gridMap.ts. Geometry values are tenths of a cell (0-10).

export type SegmentDirection = "east" | "north";
export type SegmentKind = "wall" | "door" | "window";

// A wall, door or window inside one cell; segments[i] of a cell is stored in slot i.
export interface Segment {
  kind: SegmentKind;
  direction: SegmentDirection; // east runs along y, north runs along x
  offset: number; // across the segment, from the cell's low x/y edge
  thickness: number; // size across the segment
  length: number; // size along the segment
  start: number; // along the segment, from the cell's low x/y edge
}

export interface CellSpec {
  solid?: boolean; // full-cell block; cannot be combined with anything else
  floor?: boolean;
  ceiling?: boolean; // ceiling underside and roof top
  road?: boolean;
  segments?: Segment[]; // at most 3; windows only at index 0 and 1
}

const SLOT_DOOR_BITS = [5, 4, 4];
const SLOT_DIRECTION_BITS = [6, 7, 8];
const SLOT_WINDOW_BITS = [9, 10];
const SEGMENT_COUNT_SHIFT = 12;
const SLOT_SHIFTS = [16, 32, 48];

const solid = (): CellSpec => ({ solid: true });

const bit = (index: number) => 1n << BigInt(index);

const tenths = (value: number, name: string): bigint => {
  if (!Number.isInteger(value) || value < 0 || value > 10) {
    throw new Error(`${name} must be an integer from 0 to 10, got ${value}`);
  }
  return BigInt(value);
};

export function encodeCell(spec: CellSpec): bigint {
  const segments = spec.segments ?? [];

  if (spec.solid) {
    if (spec.floor || spec.ceiling || spec.road || segments.length) {
      throw new Error("A solid cell cannot have other flags or segments");
    }
    return 1n;
  }
  if (segments.length > 3) {
    throw new Error(`A cell holds at most 3 segments, got ${segments.length}`);
  }
  if (
    segments.length === 3 &&
    (segments[1].kind === "door") !== (segments[2].kind === "door")
  ) {
    throw new Error("Segments 1 and 2 share one door bit; both or neither must be doors");
  }

  let value = 0n;
  // Bit 0 marks cells with segments; the minimap colours them by it.
  if (segments.length > 0) value |= bit(0);
  if (spec.floor) value |= bit(1);
  if (spec.ceiling) value |= bit(2);
  if (spec.road) value |= bit(3);
  value |= BigInt(segments.length) << BigInt(SEGMENT_COUNT_SHIFT);

  segments.forEach((s, index) => {
    if (s.kind === "door") value |= bit(SLOT_DOOR_BITS[index]);
    if (s.kind === "window") {
      if (index >= SLOT_WINDOW_BITS.length) {
        throw new Error("Only segments 0 and 1 can be windows");
      }
      value |= bit(SLOT_WINDOW_BITS[index]);
    }
    if (s.direction === "north") value |= bit(SLOT_DIRECTION_BITS[index]);

    const shift = BigInt(SLOT_SHIFTS[index]);
    value |= tenths(s.offset, "offset") << shift;
    value |= tenths(s.thickness, "thickness") << (shift + 4n);
    value |= tenths(s.length, "length") << (shift + 8n);
    value |= tenths(s.start, "start") << (shift + 12n);
  });

  return value;
}

export function decodeCell(value: bigint): CellSpec {
  if (value === 1n) return solid();

  const has = (index: number) => (value & bit(index)) !== 0n;
  const nibble = (shift: number) => Number((value >> BigInt(shift)) & 0xfn);
  const count = nibble(SEGMENT_COUNT_SHIFT);

  const segments: Segment[] = [];
  for (let index = 0; index < Math.min(count, 3); index++) {
    const shift = SLOT_SHIFTS[index];
    // The door bit wins over the window bit, as in the Rust renderer.
    const kind: SegmentKind = has(SLOT_DOOR_BITS[index])
      ? "door"
      : index < SLOT_WINDOW_BITS.length && has(SLOT_WINDOW_BITS[index])
        ? "window"
        : "wall";
    segments.push({
      kind,
      direction: has(SLOT_DIRECTION_BITS[index]) ? "north" : "east",
      offset: nibble(shift),
      thickness: nibble(shift + 4),
      length: nibble(shift + 8),
      start: nibble(shift + 12),
    });
  }

  const spec: CellSpec = {};
  if (has(1)) spec.floor = true;
  if (has(2)) spec.ceiling = true;
  if (has(3)) spec.road = true;
  if (segments.length) spec.segments = segments;
  return spec;
}
