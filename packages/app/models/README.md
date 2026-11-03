# Model recipes

The women in `src/assets/models/` are baked from rigged glTF characters with
[posebake](https://crates.io/crates/posebake). Each `.toml` here is a posebake recipe that
writes one OBJ + MTL pair into `src/assets/models/`.

| Recipe | Output |
|---|---|
| `woman-peasant.toml` | Peasant outfit, base body head and eyes, long hair; arms lowered |
| `woman-ranger.toml` | Ranger outfit, base body head and eyes; arms lowered |

## Re-baking

1. Install posebake: `cargo install posebake`.
2. Put the source files in `sources/` (gitignored, about 20 MB). All are CC0 packs from [Quaternius](https://quaternius.com):
   - Modular Character Outfits - Fantasy,
     `Exports/glTF (Godot-Unreal)/Outfits/`: `Female_Peasant.gltf`, `Female_Peasant.bin`,
     `Female_Ranger.gltf`, `Female_Ranger.bin`; and from `Exports/glTF (Godot-Unreal)/Modular Parts/`:
     `T_Peasant_BaseColor.png`, `T_Ranger_BaseColor.png`, `T_Regular_Female_Dark_BaseColor.png`.
   - Universal Base Characters,
     `Base Characters/Godot - UE/`: `Superhero_Female_FullBody.gltf`, `.bin`,
     `T_Superhero_Female_Dark_BaseColor.png`, `T_Eye_Brown.png`, `T_Hair_2_BaseColor.png`;
     and `Hairstyles/Rigged to Head Bone/glTF (Godot -Unreal)/`: `Hair_Long.gltf`, `.bin`.

   Only base colour textures are needed; normal and roughness maps are not read.
3. Run `pnpm run bake:models` from the repository root.

The baked files are committed, so this is only needed to change a model. Output is
deterministic: re-baking unchanged recipes reproduces the committed files.
