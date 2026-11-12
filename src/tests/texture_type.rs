use super::*;

// Both matches are exhaustive, so a variant added to either enum without the other stops
// this file from compiling; the test then checks the values agree.
fn to_engine(texture: TextureType) -> engine::TextureType {
    match texture {
        TextureType::WALL => engine::TextureType::WALL,
        TextureType::CEILING => engine::TextureType::CEILING,
        TextureType::FLOOR => engine::TextureType::FLOOR,
        TextureType::ROAD => engine::TextureType::ROAD,
        TextureType::DOOR => engine::TextureType::DOOR,
        TextureType::TREE_CONE => engine::TextureType::TREE_CONE,
        TextureType::PILLAR => engine::TextureType::PILLAR,
        TextureType::BUSH1 => engine::TextureType::BUSH1,
        TextureType::TREE_VASE => engine::TextureType::TREE_VASE,
        TextureType::TREE_COLUMNAR => engine::TextureType::TREE_COLUMNAR,
        TextureType::LADY => engine::TextureType::LADY,
        TextureType::WINDOW => engine::TextureType::WINDOW,
        TextureType::ROOF => engine::TextureType::ROOF,
    }
}

fn to_game(texture: &engine::TextureType) -> TextureType {
    match texture {
        engine::TextureType::WALL => TextureType::WALL,
        engine::TextureType::CEILING => TextureType::CEILING,
        engine::TextureType::FLOOR => TextureType::FLOOR,
        engine::TextureType::ROAD => TextureType::ROAD,
        engine::TextureType::DOOR => TextureType::DOOR,
        engine::TextureType::TREE_CONE => TextureType::TREE_CONE,
        engine::TextureType::PILLAR => TextureType::PILLAR,
        engine::TextureType::BUSH1 => TextureType::BUSH1,
        engine::TextureType::TREE_VASE => TextureType::TREE_VASE,
        engine::TextureType::TREE_COLUMNAR => TextureType::TREE_COLUMNAR,
        engine::TextureType::LADY => TextureType::LADY,
        engine::TextureType::WINDOW => TextureType::WINDOW,
        engine::TextureType::ROOF => TextureType::ROOF,
    }
}

#[test]
fn texture_type_values_match_the_engine() {
    let variants: [fn() -> TextureType; 13] = [
        || TextureType::WALL,
        || TextureType::CEILING,
        || TextureType::FLOOR,
        || TextureType::ROAD,
        || TextureType::DOOR,
        || TextureType::TREE_CONE,
        || TextureType::PILLAR,
        || TextureType::BUSH1,
        || TextureType::TREE_VASE,
        || TextureType::TREE_COLUMNAR,
        || TextureType::LADY,
        || TextureType::WINDOW,
        || TextureType::ROOF,
    ];
    for variant in variants {
        let value = variant() as i32;
        let engine_texture = to_engine(variant());
        assert_eq!(to_game(&engine_texture) as i32, value);
        assert_eq!(engine_texture as i32, value);
    }
}
