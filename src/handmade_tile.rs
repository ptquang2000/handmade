use super::*;

#[derive(Default, Clone, Copy)]
pub struct TileMapDifference {
    pub dxy: V2,
    pub dz: f32,
}

#[derive(Default, Clone, Copy)]
pub struct TileMapPosition {
    pub abs_tile_x: u32,
    pub abs_tile_y: u32,
    pub abs_tile_z: u32,

    pub offset: V2,
}

#[derive(Default, Clone, Copy)]
pub struct TileChunkPosition {
    pub tile_chunk_x: u32,
    pub tile_chunk_y: u32,
    pub tile_chunk_z: u32,

    pub rel_tile_x: u32,
    pub rel_tile_y: u32,
}

#[derive(Default)]
pub struct TileChunk {
    pub tiles: *mut u32,
}

#[derive(Default)]
pub struct TileMap {
    pub chunk_shift: u32,
    pub chunk_mask: u32,
    pub chunk_dim: u32,

    pub tile_side_in_meters: f32,

    pub tile_chunk_count_x: u32,
    pub tile_chunk_count_y: u32,
    pub tile_chunk_count_z: u32,

    pub tile_chunks: *mut TileChunk,
}

fn get_tile_chunk(
    tile_map: &TileMap,
    tile_chunk_x: u32,
    tile_chunk_y: u32,
    tile_chunk_z: u32,
) -> Option<&mut TileChunk> {
    if (tile_chunk_x < tile_map.tile_chunk_count_x)
        && (tile_chunk_y < tile_map.tile_chunk_count_y)
        && (tile_chunk_z < tile_map.tile_chunk_count_z)
    {
        let result = tile_map.tile_chunks.wrapping_add(
            (tile_chunk_z * tile_map.tile_chunk_count_y * tile_map.tile_chunk_count_x
                + tile_chunk_y * tile_map.tile_chunk_count_x
                + tile_chunk_x) as usize,
        );
        unsafe { result.as_mut() }
    } else {
        None
    }
}

fn set_tile_value_unchecked(
    tile_map: &TileMap,
    tile_chunk: &mut TileChunk,
    tile_x: u32,
    tile_y: u32,
    tile_value: u32,
) {
    debug_assert!(tile_x < tile_map.chunk_dim);
    debug_assert!(tile_y < tile_map.chunk_dim);
    let result = tile_chunk
        .tiles
        .wrapping_add((tile_y * tile_map.chunk_dim + tile_x) as usize);
    unsafe { *result = tile_value };
}

fn get_tile_value_unchecked(
    tile_map: &TileMap,
    tile_chunk: &TileChunk,
    tile_x: u32,
    tile_y: u32,
) -> u32 {
    debug_assert!(tile_x < tile_map.chunk_dim);
    debug_assert!(tile_y < tile_map.chunk_dim);
    let tile_map_value = tile_chunk
        .tiles
        .wrapping_add((tile_y * tile_map.chunk_dim + tile_x) as usize);
    *unsafe { tile_map_value.as_ref_unchecked() }
}

impl TileChunk {
    fn get_tile_value(&self, tile_map: &TileMap, test_x: u32, test_y: u32) -> u32 {
        let mut tile_map_value = 0;
        if !self.tiles.is_null() {
            tile_map_value = get_tile_value_unchecked(tile_map, self, test_x, test_y);
        }
        tile_map_value
    }
}

fn set_tile_value_(
    tile_map: &TileMap,
    tile_chunk: &mut TileChunk,
    test_x: u32,
    test_y: u32,
    tile_value: u32,
) {
    set_tile_value_unchecked(tile_map, tile_chunk, test_x, test_y, tile_value);
}

fn get_chunk_position_for(
    tile_map: &TileMap,
    abs_tile_x: u32,
    abs_tile_y: u32,
    abs_tile_z: u32,
) -> TileChunkPosition {
    let mut result = TileChunkPosition::default();

    result.tile_chunk_x = abs_tile_x.wrapping_shr(tile_map.chunk_shift);
    result.tile_chunk_y = abs_tile_y.wrapping_shr(tile_map.chunk_shift);
    result.tile_chunk_z = abs_tile_z;
    result.rel_tile_x = abs_tile_x & tile_map.chunk_mask;
    result.rel_tile_y = abs_tile_y & tile_map.chunk_mask;

    result
}

pub fn get_tile_value(
    tile_map: &TileMap,
    abs_tile_x: u32,
    abs_tile_y: u32,
    abs_tile_z: u32,
) -> u32 {
    let chunk_pos = get_chunk_position_for(tile_map, abs_tile_x, abs_tile_y, abs_tile_z);
    let mut tile_chunk_value = 0;
    if let Some(tile_chunk) = get_tile_chunk(
        tile_map,
        chunk_pos.tile_chunk_x,
        chunk_pos.tile_chunk_y,
        chunk_pos.tile_chunk_z,
    ) {
        tile_chunk_value =
            tile_chunk.get_tile_value(tile_map, chunk_pos.rel_tile_x, chunk_pos.rel_tile_y);
    }
    return tile_chunk_value;
}

impl TileMapPosition {
    pub fn get_tile_value(&self, tile_map: &TileMap) -> u32 {
        let tile_chunk_value =
            get_tile_value(tile_map, self.abs_tile_x, self.abs_tile_y, self.abs_tile_z);
        tile_chunk_value
    }
}

pub fn is_tile_value_empty(tile_value: u32) -> bool {
    let empty = (tile_value == 1) || (tile_value == 3) || (tile_value == 4);

    empty
}

pub fn is_tile_map_point_empty(tile_map: &TileMap, pos: TileMapPosition) -> bool {
    let tile_chunk_value = get_tile_value(tile_map, pos.abs_tile_x, pos.abs_tile_y, pos.abs_tile_z);
    let empty = (tile_chunk_value == 1) || (tile_chunk_value == 3) || (tile_chunk_value == 4);

    empty
}

pub fn set_tile_value(
    arena: &mut MemoryArena,
    tile_map: &TileMap,
    abs_tile_x: u32,
    abs_tile_y: u32,
    abs_tile_z: u32,
    value: u32,
) {
    let chunk_pos = get_chunk_position_for(tile_map, abs_tile_x, abs_tile_y, abs_tile_z);
    let tile_chunk = get_tile_chunk(
        tile_map,
        chunk_pos.tile_chunk_x,
        chunk_pos.tile_chunk_y,
        chunk_pos.tile_chunk_z,
    )
    .unwrap();

    if tile_chunk.tiles.is_null() {
        let tile_count = (tile_map.chunk_dim * tile_map.chunk_dim) as usize;
        tile_chunk.tiles = push_array!(arena, tile_count, u32);
        let tiles = unsafe { std::slice::from_raw_parts_mut(tile_chunk.tiles, tile_count) };
        tiles.fill(1);
    }

    set_tile_value_(
        tile_map,
        tile_chunk,
        chunk_pos.rel_tile_x,
        chunk_pos.rel_tile_y,
        value,
    );
}

fn recanonicalize_coord(tile_map: &TileMap, tile: &mut u32, tile_rel: &mut f32) {
    let offset = round_f32_to_i32(*tile_rel / tile_map.tile_side_in_meters as f32);
    *tile = tile.wrapping_add(offset as u32);
    *tile_rel -= offset as f32 * tile_map.tile_side_in_meters;

    debug_assert!(*tile_rel >= -0.5 * tile_map.tile_side_in_meters);
    debug_assert!(*tile_rel <= 0.5 * tile_map.tile_side_in_meters);
}

pub fn recanonicalize_position(tile_map: &TileMap, pos: TileMapPosition) -> TileMapPosition {
    let mut result = pos;

    recanonicalize_coord(tile_map, &mut result.abs_tile_x, &mut result.offset.x);
    recanonicalize_coord(tile_map, &mut result.abs_tile_y, &mut result.offset.y);

    result
}

pub fn are_on_same_tile(a: &TileMapPosition, b: &TileMapPosition) -> bool {
    let result = (a.abs_tile_x == b.abs_tile_x)
        && (a.abs_tile_y == b.abs_tile_y)
        && (a.abs_tile_z == b.abs_tile_z);

    result
}

pub fn subtract(tile_map: &TileMap, a: TileMapPosition, b: TileMapPosition) -> TileMapDifference {
    let mut result = TileMapDifference::default();

    let dtile_xy = V2 {
        x: (a.abs_tile_x.wrapping_sub(b.abs_tile_x) as i32) as f32,
        y: (a.abs_tile_y.wrapping_sub(b.abs_tile_y) as i32) as f32,
    };
    let dtile_z = a.abs_tile_z.wrapping_sub(b.abs_tile_z) as i32 as f32;

    result.dxy = tile_map.tile_side_in_meters * dtile_xy + (a.offset - b.offset);
    result.dz = tile_map.tile_side_in_meters * dtile_z + 0.0;

    result
}

pub fn centered_tile_point(abs_tile_x: u32, abs_tile_y: u32, abs_tile_z: u32) -> TileMapPosition {
    let mut result = TileMapPosition::default();

    result.abs_tile_x = abs_tile_x;
    result.abs_tile_y = abs_tile_y;
    result.abs_tile_z = abs_tile_z;

    result
}
