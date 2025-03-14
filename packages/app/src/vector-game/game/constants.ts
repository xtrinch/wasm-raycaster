import { Noise } from "noisejs";
import seedrandom from "seedrandom";
import { createNoise2D } from "simplex-noise";

export const CIRCLE = Math.PI * 2;
export const SEED = "fixed-seed1";
export const rng = seedrandom(SEED);
export const noise2D = createNoise2D(rng);
export const perlinNoise = new Noise("fixed-seed1");
