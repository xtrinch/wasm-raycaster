// Minimal Wavefront OBJ reader: vertex positions, normals, faces and material colours
// (diffuse Kd from an MTL file). Texture coordinates are ignored; polygons are
// fan-triangulated.

export type Color = [number, number, number]; // 0-1

export interface Mesh {
  positions: number[]; // x, y, z per vertex, in the file's own axes (OBJ is usually Y up)
  triangles: number[]; // three vertex indices per triangle
  normals: number[]; // x, y, z per triangle corner, aligned with triangles
  colors: (Color | null)[]; // per triangle, from its material; null without one
}

// Diffuse colours by material name from an MTL file.
export function parseMtl(text: string): Record<string, Color> {
  const colors: Record<string, Color> = {};
  let current: string | null = null;
  for (const rawLine of text.split("\n")) {
    const parts = rawLine.trim().split(/\s+/);
    if (parts[0] === "newmtl") current = parts.slice(1).join(" ");
    if (parts[0] === "Kd" && current !== null) {
      colors[current] = [Number(parts[1]), Number(parts[2]), Number(parts[3])];
    }
  }
  return colors;
}

export function parseObj(text: string, materials: Record<string, Color> = {}): Mesh {
  const positions: number[] = [];
  const fileNormals: number[] = [];
  const corners: { vertex: number; normal: number | null }[] = [];
  const colors: (Color | null)[] = [];
  let color: Color | null = null;

  const resolve = (value: string, count: number, lineIndex: number) => {
    const index = parseInt(value, 10);
    // Negative indices count back from the latest entry.
    const resolved = index < 0 ? count + index : index - 1;
    if (!Number.isInteger(resolved) || resolved < 0 || resolved >= count) {
      throw new Error(`OBJ line ${lineIndex + 1}: bad index "${value}"`);
    }
    return resolved;
  };

  text.split("\n").forEach((rawLine, lineIndex) => {
    const parts = rawLine.trim().split(/\s+/);
    if (parts[0] === "v") {
      positions.push(Number(parts[1]), Number(parts[2]), Number(parts[3]));
    } else if (parts[0] === "vn") {
      fileNormals.push(Number(parts[1]), Number(parts[2]), Number(parts[3]));
    } else if (parts[0] === "usemtl") {
      color = materials[parts.slice(1).join(" ")] ?? null;
    } else if (parts[0] === "f") {
      // Each corner is "v", "v/vt", "v//vn" or "v/vt/vn".
      const face = parts.slice(1).map((corner) => {
        const [vertex, , normal] = corner.split("/");
        return {
          vertex: resolve(vertex, positions.length / 3, lineIndex),
          normal: normal ? resolve(normal, fileNormals.length / 3, lineIndex) : null,
        };
      });
      for (let i = 1; i + 1 < face.length; i++) {
        corners.push(face[0], face[i], face[i + 1]);
        colors.push(color);
      }
    }
  });

  // Corners without a file normal get the area-weighted average of the faces around
  // their vertex, which shades curved surfaces smoothly.
  const averaged = new Array(positions.length).fill(0);
  for (let i = 0; i < corners.length; i += 3) {
    const [a, b, c] = [corners[i], corners[i + 1], corners[i + 2]].map((corner) => corner.vertex * 3);
    const u = [0, 1, 2].map((k) => positions[b + k] - positions[a + k]);
    const v = [0, 1, 2].map((k) => positions[c + k] - positions[a + k]);
    const face = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    for (const vertex of [a, b, c]) {
      for (let k = 0; k < 3; k++) averaged[vertex + k] += face[k];
    }
  }

  const normals: number[] = [];
  for (const corner of corners) {
    const source = corner.normal !== null ? fileNormals : averaged;
    const start = (corner.normal !== null ? corner.normal : corner.vertex) * 3;
    const [x, y, z] = [source[start], source[start + 1], source[start + 2]];
    const length = Math.hypot(x, y, z) || 1;
    normals.push(x / length, y / length, z / length);
  }

  return { positions, triangles: corners.map((corner) => corner.vertex), normals, colors };
}
