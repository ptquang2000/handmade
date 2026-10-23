#[path = "handmade_platform.rs"]
#[macro_use]
mod platform;
pub use self::platform::*;

pub struct MemoryArena {
    size: MemoryIndex,
    base: *mut u8,
    used: MemoryIndex,
}

fn initialize_arena(arena: &mut MemoryArena, size: MemoryIndex, base: *mut u8) {
    arena.size = size;
    arena.base = base;
    arena.used = 0;
}

macro_rules! push_struct {
    ($arena:expr, $type:ty) => {
        push_size_($arena, std::mem::size_of::<$type>()).cast::<$type>()
    };
}
macro_rules! push_array {
    ($arena:expr, $count:expr, $type:ty) => {
        push_size_($arena, $count as usize * std::mem::size_of::<$type>()).cast::<$type>()
    };
}
pub fn push_size_(arena: &mut MemoryArena, size: MemoryIndex) -> *mut u8 {
    debug_assert!(arena.used + size <= arena.size);
    let result = arena.base.wrapping_add(arena.used);
    arena.used += size;
    result
}

#[path = "handmade_math.rs"]
pub mod math;
use self::math::*;

#[path = "handmade_intrinsics.rs"]
mod intrinsics;
use self::intrinsics::*;

#[path = "handmade_random.rs"]
mod random;
use self::random::*;

#[path = "handmade_tile.rs"]
pub mod tile;
use self::tile::*;

#[derive(Default)]
struct World {
    tile_map: *mut TileMap,
}

#[derive(Default)]
struct LoadedBitmap {
    width: i32,
    height: i32,
    pixels: *mut u8,
}

impl LoadedBitmap {
    fn pixels(&self, bytes_per_pixel: i32) -> &mut [u8] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self.pixels,
                (self.height * self.width * bytes_per_pixel) as usize,
            )
        }
    }
}

#[derive(Default)]
pub struct HeroBitmaps {
    align_x: i32,
    align_y: i32,
    head: LoadedBitmap,
    cape: LoadedBitmap,
    torso: LoadedBitmap,
}

#[derive(Default)]
struct Entity {
    exists: bool,
    p: TileMapPosition,
    dp: V2,
    facing_direction: u32,
    width: f32,
    height: f32,
}

pub struct GameState {
    world_arena: MemoryArena,
    world: *mut World,

    camera_following_entity_index: usize,
    camera_p: TileMapPosition,

    player_index_for_controllers: [usize; array_len!(Controllers)],
    entity_count: usize,
    entities: [Entity; 256],

    back_drop: LoadedBitmap,
    hero_bitmaps: [HeroBitmaps; 4],
}

impl GameMemory {
    pub fn get_game_state(&self) -> &mut GameState {
        assert!(std::mem::size_of::<GameState>() <= self.transient_storage_size as usize);
        unsafe { &mut *(self.permanent_storage_memory as *mut GameState) }
    }

    pub fn read_entire_file(
        &self,
        thread: &ThreadContext,
        filename: &str,
    ) -> Option<(*mut (), i64)> {
        if let Some(func) = self.read_entire_file_stub {
            func(thread, filename)
        } else {
            None
        }
    }

    pub fn write_entire_file(
        &self,
        thread: &ThreadContext,
        filename: &str,
        memory: *mut (),
        memory_size: i64,
    ) -> bool {
        if let Some(func) = self.write_entire_file_stub {
            func(thread, filename, memory, memory_size)
        } else {
            false
        }
    }

    fn free_file_memory(&self, thread: &ThreadContext, memory: *mut (), size: i64) {
        if let Some(func) = self.free_file_memory_stub {
            func(thread, memory, size)
        }
    }
}

fn draw_rectangle(
    buffer: &mut GameOffscreenBuffer,
    real_min: V2,
    real_max: V2,
    r: f32,
    g: f32,
    b: f32,
) {
    let max_x = round_f32_to_i32(real_max.x).min(buffer.width);
    let max_y = round_f32_to_i32(real_max.y).min(buffer.height);
    let min_x = round_f32_to_i32(real_min.x).max(0).min(max_x);
    let min_y = round_f32_to_i32(real_min.y).max(0).min(max_y);

    let color = round_f32_to_u32(r * u8::MAX as f32) << 16
        | round_f32_to_u32(g * u8::MAX as f32) << 8
        | round_f32_to_u32(b * u8::MAX as f32);

    for rows in buffer
        .as_slice_mut()
        .chunks_exact_mut(buffer.pitch as usize)
        .take(max_y as usize)
        .skip(min_y as usize)
    {
        for pixel in rows
            .chunks_exact_mut(buffer.bytes_per_pixel as usize)
            .take(max_x as usize)
            .skip(min_x as usize)
        {
            pixel.copy_from_slice(&color.to_ne_bytes());
        }
    }
}

fn draw_bitmap(
    buffer: &mut GameOffscreenBuffer,
    bitmap: &LoadedBitmap,
    real_x: f32,
    real_y: f32,
    align_x: i32,
    align_y: i32,
) {
    let real_x = real_x - align_x as f32;
    let real_y = real_y - align_y as f32;

    let min_x = round_f32_to_i32(real_x).max(0) as usize;
    let min_y = round_f32_to_i32(real_y).max(0) as usize;
    let max_x = round_f32_to_i32(real_x + bitmap.width as f32).min(buffer.width) as usize;
    let max_y = round_f32_to_i32(real_y + bitmap.height as f32).min(buffer.height) as usize;

    let source_offset_x = -round_f32_to_i32(real_x).min(0);
    let source_offset_y = -round_f32_to_i32(real_y).min(0);

    let source = bitmap
        .pixels(buffer.bytes_per_pixel)
        .chunks_exact_mut((bitmap.width * buffer.bytes_per_pixel) as usize)
        .rev()
        .skip(source_offset_y as usize);
    let dest = buffer
        .as_slice_mut()
        .chunks_exact_mut(buffer.pitch as usize);

    for (dest_row, source_row) in dest.take(max_y).skip(min_y).zip(source) {
        for (dest, source) in dest_row
            .chunks_exact_mut(buffer.bytes_per_pixel as usize)
            .take(max_x)
            .skip(min_x)
            .zip(
                source_row
                    .chunks_exact(buffer.bytes_per_pixel as usize)
                    .skip(source_offset_x as usize),
            )
        {
            let source_color = u32::from_ne_bytes([source[0], source[1], source[2], source[3]]);
            let a = ((source_color >> 24) & 0xFF) as f32 / 255.0;
            let sr = ((source_color >> 16) & 0xFF) as f32;
            let sg = ((source_color >> 8) & 0xFF) as f32;
            let sb = ((source_color >> 0) & 0xFF) as f32;

            let dest_color = u32::from_ne_bytes([dest[0], dest[1], dest[2], dest[3]]);
            let dr = ((dest_color >> 16) & 0xFF) as f32;
            let dg = ((dest_color >> 8) & 0xFF) as f32;
            let db = ((dest_color >> 0) & 0xFF) as f32;

            let r = (1.0 - a) * dr + a * sr;
            let g = (1.0 - a) * dg + a * sg;
            let b = (1.0 - a) * db + a * sb;
            dest.copy_from_slice(
                &((((r + 0.5) as u32) << 16)
                    | (((g + 0.5) as u32) << 8)
                    | (((b + 0.5) as u32) << 0))
                    .to_ne_bytes(),
            );
        }
    }
}

#[repr(C, packed(1))]
#[derive(Debug)]
struct BitmapHeader {
    filetype: u16,
    filesize: u32,
    reserved1: u16,
    reserved2: u16,
    bitmap_offset: u32,
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bits_per_pixel: u16,
    compression: u32,
    size_of_bitmap: u32,
    horz_resolution: i32,
    vert_resolution: i32,
    color_user: u32,
    color_important: u32,

    red_mask: u32,
    green_mask: u32,
    blue_mask: u32,
}

fn debug_load_bmp(
    thread: &ThreadContext,
    read_entire_file: DebugPlatformReadEntireFile,
    file_name: &str,
) -> Option<LoadedBitmap> {
    // NOTE: byte order in memory BB GG RR AA
    // in little endian 0xAARRGGBB
    if let Some((content, size)) = read_entire_file(thread, file_name) {
        if size != 0 {
            let header = unsafe { content.cast::<BitmapHeader>().as_ref_unchecked() };
            let pixels = content.wrapping_byte_add(header.bitmap_offset as usize);

            let mut result = LoadedBitmap::default();
            result.pixels = pixels as _;
            result.width = header.width;
            result.height = header.height;

            debug_assert!(header.compression == 3);

            let red_mask = header.red_mask;
            let green_mask = header.green_mask;
            let blue_mask = header.blue_mask;
            let alpha_mask = !(red_mask | green_mask | blue_mask);

            let red_scan = red_mask.trailing_zeros();
            let green_scan = green_mask.trailing_zeros();
            let blue_scan = blue_mask.trailing_zeros();
            let alpha_scan = alpha_mask.trailing_zeros();

            let red_shift = 16 - red_scan as i32;
            let green_shift = 8 - green_scan as i32;
            let blue_shift = 0 - blue_scan as i32;
            let alpha_shift = 24 - alpha_scan as i32;

            let bytes_per_pixel = std::mem::size_of::<u32>();
            for pixel in result
                .pixels(bytes_per_pixel as i32)
                .chunks_exact_mut(bytes_per_pixel)
            {
                let c = u32::from_ne_bytes([pixel[0], pixel[1], pixel[2], pixel[3]]);

                if cfg!(any()) {
                    pixel.copy_from_slice(
                        &((((c >> alpha_scan) & 0xFF) << 24)
                            | (((c >> red_scan) & 0xFF) << 16)
                            | (((c >> green_scan) & 0xFF) << 8)
                            | (((c >> blue_scan) & 0xFF) << 0))
                            .to_ne_bytes(),
                    );
                } else {
                    pixel.copy_from_slice(
                        &((c & red_mask).rotate_left(red_shift as u32)
                            | (c & green_mask).rotate_left(green_shift as u32)
                            | (c & blue_mask).rotate_left(blue_shift as u32)
                            | (c & alpha_mask).rotate_left(alpha_shift as u32))
                        .to_ne_bytes(),
                    );
                }
            }

            return Some(result);
        }
    }

    None
}

fn get_entity(game_state: &mut GameState, index: usize) -> Option<&mut Entity> {
    if index > 0 && index < game_state.entities.len() {
        return Some(&mut game_state.entities[index]);
    }
    None
}

fn initialize_player(game_state: &mut GameState, entity_index: usize) {
    let entity = get_entity(game_state, entity_index).unwrap();

    entity.exists = true;
    entity.p.abs_tile_x = 1;
    entity.p.abs_tile_y = 3;
    entity.p.offset_ = V2::default();
    entity.height = 0.5;
    entity.width = 1.0;

    if let None = get_entity(game_state, game_state.camera_following_entity_index) {
        game_state.camera_following_entity_index = entity_index;
    }
}

fn add_entity(game_state: &mut GameState) -> usize {
    let entity_index = game_state.entity_count;

    game_state.entity_count += 1;
    debug_assert!(game_state.entity_count < game_state.entities.len());
    let entity = &mut game_state.entities[entity_index];
    *entity = Entity::default();

    entity_index
}

fn test_wall(
    wall_x: f32,
    rel_x: f32,
    rel_y: f32,
    player_delta_x: f32,
    player_delta_y: f32,
    t_min: &mut f32,
    min_y: f32,
    max_y: f32,
) -> bool {
    let mut hit = false;

    let t_epsilon = 0.00001;
    if player_delta_x != 0.0 {
        let t_result = (wall_x - rel_x) / player_delta_x;
        let y = rel_y + t_result * player_delta_y;
        if t_result >= 0.0 && *t_min > t_result {
            if y >= min_y && y <= max_y {
                *t_min = (t_result - t_epsilon).max(0.0);
                hit = true;
            }
        }
    }

    hit
}

fn move_player(game_state: &mut GameState, entity: &mut Entity, dt: f32, mut ddp: V2) {
    let tile_map = unsafe {
        game_state
            .world
            .as_ref_unchecked()
            .tile_map
            .as_ref_unchecked()
    };

    let ddp_length = ddp.length_sq();
    if ddp_length > 1.0 {
        ddp *= 1.0 / square_root(ddp_length);
    }

    let mut player_speed = 50.0;
    ddp *= player_speed;

    ddp += -8.0 * entity.dp;

    let old_player_p = entity.p;
    let mut player_delta = 0.5 * ddp * square(dt) + entity.dp * dt;
    entity.dp = ddp * dt + entity.dp;
    let mut new_player_p = old_player_p.offset(tile_map, player_delta);

    // cfg_select! {
    match true {
        false => {
            let mut player_left = new_player_p;
            player_left.offset_.x -= 0.5 * entity.width;
            player_left = recanonicalize_position(tile_map, player_left);

            let mut player_right = new_player_p;
            player_right.offset_.x += 0.5 * entity.width;
            player_right = recanonicalize_position(tile_map, player_right);

            let mut collided = false;
            let mut col_p = TileMapPosition::default();
            if !is_tile_map_point_empty(tile_map, new_player_p) {
                col_p = new_player_p;
                collided = true;
            }
            if !is_tile_map_point_empty(tile_map, player_left) {
                col_p = player_left;
                collided = true;
            }
            if !is_tile_map_point_empty(tile_map, player_right) {
                col_p = player_right;
                collided = true;
            }

            if collided {
                let mut r = V2::default();
                if col_p.abs_tile_x < entity.p.abs_tile_x {
                    r = V2 { x: 1., y: 0. };
                }
                if col_p.abs_tile_x > entity.p.abs_tile_x {
                    r = V2 { x: -1., y: 0. };
                }
                if col_p.abs_tile_y < entity.p.abs_tile_y {
                    r = V2 { x: 0., y: 1. };
                }
                if col_p.abs_tile_y > entity.p.abs_tile_y {
                    r = V2 { x: 0., y: -1. };
                }

                entity.dp = entity.dp - 1. * entity.dp.inner(r) * r;
            } else {
                entity.p = new_player_p;
            }
        }
        _ => {
            let mut min_tile_x = old_player_p.abs_tile_x.min(new_player_p.abs_tile_x);
            let mut min_tile_y = old_player_p.abs_tile_y.min(new_player_p.abs_tile_y);
            let mut max_tile_x = old_player_p.abs_tile_x.max(new_player_p.abs_tile_x);
            let mut max_tile_y = old_player_p.abs_tile_y.max(new_player_p.abs_tile_y);

            let entity_tile_width =
                ceil_f32_to_i32(entity.width / tile_map.tile_side_in_meters) as u32;
            let entity_tile_height =
                ceil_f32_to_i32(entity.height / tile_map.tile_side_in_meters) as u32;

            min_tile_x -= entity_tile_width;
            min_tile_y -= entity_tile_height;
            max_tile_x += entity_tile_width;
            max_tile_y += entity_tile_height;

            let abs_tile_z = entity.p.abs_tile_z;

            let mut t_remaining = 1.;
            for iterator in 0..4 {
                if t_remaining <= 0.0 {
                    break;
                }

                let mut t_min = t_remaining;
                let mut wall_normal = V2::default();

                debug_assert!((max_tile_x - min_tile_x) < 32);
                debug_assert!((max_tile_y - min_tile_y) < 32);

                for abs_tile_y in min_tile_y..=max_tile_y {
                    for abs_tile_x in min_tile_x..=max_tile_x {
                        let test_tile_p = centered_tile_point(abs_tile_x, abs_tile_y, abs_tile_z);
                        let tile_value = test_tile_p.get_tile_value(tile_map);
                        if !is_tile_value_empty(tile_value) {
                            let diameter_w = tile_map.tile_side_in_meters + entity.width;
                            let diameter_h = tile_map.tile_side_in_meters + entity.height;
                            let min_corner = -0.5
                                * V2 {
                                    x: diameter_w,
                                    y: diameter_h,
                                };
                            let max_corner = 0.5
                                * V2 {
                                    x: diameter_w,
                                    y: diameter_h,
                                };

                            let rel_old_player_p = subtract(tile_map, entity.p, test_tile_p);
                            let rel = rel_old_player_p.dxy;

                            if test_wall(
                                min_corner.x,
                                rel.x,
                                rel.y,
                                player_delta.x,
                                player_delta.y,
                                &mut t_min,
                                min_corner.y,
                                max_corner.y,
                            ) {
                                wall_normal = V2 { x: -1., y: 0. };
                            }
                            if test_wall(
                                max_corner.x,
                                rel.x,
                                rel.y,
                                player_delta.x,
                                player_delta.y,
                                &mut t_min,
                                min_corner.y,
                                max_corner.y,
                            ) {
                                wall_normal = V2 { x: 1., y: 0. };
                            }
                            if test_wall(
                                min_corner.y,
                                rel.y,
                                rel.x,
                                player_delta.y,
                                player_delta.x,
                                &mut t_min,
                                min_corner.x,
                                max_corner.x,
                            ) {
                                wall_normal = V2 { x: 0., y: -1. };
                            }
                            if test_wall(
                                max_corner.y,
                                rel.y,
                                rel.x,
                                player_delta.y,
                                player_delta.x,
                                &mut t_min,
                                min_corner.x,
                                max_corner.x,
                            ) {
                                wall_normal = V2 { x: 0., y: 1. };
                            }
                        }
                    }
                }

                entity.p = entity.p.offset(tile_map, t_min * player_delta);
                entity.dp = entity.dp - 1. * entity.dp.inner(wall_normal) * wall_normal;
                player_delta = player_delta - 1. * player_delta.inner(wall_normal) * wall_normal;
                t_remaining -= t_min;
            }
        }
    }

    if !are_on_same_tile(&old_player_p, &entity.p) {
        let new_tile_value = entity.p.get_tile_value(tile_map);

        if new_tile_value == 3 {
            entity.p.abs_tile_z += 1;
        } else if new_tile_value == 4 {
            entity.p.abs_tile_z -= 1;
        }
    }

    if entity.dp.x == 0.0 && entity.dp.y == 0.0 {
    } else if absolute_value(entity.dp.x) > absolute_value(entity.dp.y) {
        if entity.dp.x > 0. {
            entity.facing_direction = 0;
        } else {
            entity.facing_direction = 2;
        }
    } else {
        if entity.dp.y > 0. {
            entity.facing_direction = 1;
        } else {
            entity.facing_direction = 3;
        }
    }
}

#[no_mangle]
extern "C" fn game_update_and_render(
    thread: &ThreadContext,
    memory: &mut GameMemory,
    inputs: &mut GameInput,
    buffer: &mut GameOffscreenBuffer,
) {
    if !memory.is_initialized {
        let game_state = memory.get_game_state();

        add_entity(game_state);

        game_state.back_drop = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_background.bmp\0",
        )
        .unwrap();

        let mut bitmap = game_state.hero_bitmaps.iter_mut();
        let hero = bitmap.next().unwrap();
        hero.head = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_right_head.bmp\0",
        )
        .unwrap();
        hero.cape = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_right_cape.bmp\0",
        )
        .unwrap();
        hero.torso = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_right_torso.bmp\0",
        )
        .unwrap();
        hero.align_x = 72;
        hero.align_y = 182;

        let hero = bitmap.next().unwrap();
        hero.head = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_back_head.bmp\0",
        )
        .unwrap();
        hero.cape = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_back_cape.bmp\0",
        )
        .unwrap();
        hero.torso = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_back_torso.bmp\0",
        )
        .unwrap();
        hero.align_x = 72;
        hero.align_y = 182;

        let mut hero = bitmap.next().unwrap();
        hero.head = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_left_head.bmp\0",
        )
        .unwrap();
        hero.cape = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_left_cape.bmp\0",
        )
        .unwrap();
        hero.torso = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_left_torso.bmp\0",
        )
        .unwrap();
        hero.align_x = 72;
        hero.align_y = 182;

        let hero = bitmap.next().unwrap();
        hero.head = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_front_head.bmp\0",
        )
        .unwrap();
        hero.cape = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_front_cape.bmp\0",
        )
        .unwrap();
        hero.torso = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_hero_front_torso.bmp\0",
        )
        .unwrap();
        hero.align_x = 72;
        hero.align_y = 182;

        game_state.camera_p.abs_tile_x = 17 / 2;
        game_state.camera_p.abs_tile_y = 9 / 2;

        initialize_arena(
            &mut game_state.world_arena,
            memory.permanent_storage_size - std::mem::size_of_val(memory.get_game_state()),
            memory
                .permanent_storage_memory
                .wrapping_add(std::mem::size_of_val(memory.get_game_state())),
        );

        game_state.world = push_struct!(&mut game_state.world_arena, World);
        let world = unsafe { game_state.world.as_mut_unchecked() };
        world.tile_map = push_struct!(&mut game_state.world_arena, TileMap);

        let tile_map = unsafe { world.tile_map.as_mut_unchecked() };

        tile_map.chunk_shift = 4;
        tile_map.chunk_mask = (1 << tile_map.chunk_shift) - 1;
        tile_map.chunk_dim = 1 << tile_map.chunk_shift;

        tile_map.tile_chunk_count_x = 128;
        tile_map.tile_chunk_count_y = 128;
        tile_map.tile_chunk_count_z = 2;
        let chunk_count = (tile_map.tile_chunk_count_x
            * tile_map.tile_chunk_count_y
            * tile_map.tile_chunk_count_z) as usize;
        tile_map.tile_chunks = push_array!(&mut game_state.world_arena, chunk_count, TileChunk);

        tile_map.tile_side_in_meters = 1.4;

        let mut random_number_index = 0;
        let tiles_per_width = 17u32;
        let tiles_per_height = 9u32;
        cfg_select! {
            false => {
                let mut screen_x = i32::MAX / 2 as u32;
                let mut screen_y = i32::MAX / 2 as u32;
            }
            true => {
                let mut screen_x = 0;
                let mut screen_y = 0;
            }
        }
        let mut abs_tile_z = 0u32;

        let mut door_left = false;
        let mut door_right = false;
        let mut door_top = false;
        let mut door_bottom = false;
        let mut door_up = false;
        let mut door_down = false;
        for screen_index in 0..100 {
            debug_assert!(random_number_index < RANDOM_NUMBER_TABLE.len());
            let random_choice = if door_up || door_down {
                RANDOM_NUMBER_TABLE[random_number_index] % 2
            } else {
                RANDOM_NUMBER_TABLE[random_number_index] % 3
            };
            random_number_index += 1;

            let mut created_z_door = false;
            if random_choice == 2 {
                created_z_door = true;
                if abs_tile_z == 0 {
                    door_up = true;
                } else {
                    door_down = true;
                }
            } else if random_choice == 1 {
                door_right = true;
            } else {
                door_top = true;
            }

            for tile_y in 0..tiles_per_height {
                for tile_x in 0..tiles_per_width {
                    let abs_tile_x = screen_x * tiles_per_width + tile_x;
                    let abs_tile_y = screen_y * tiles_per_height + tile_y;

                    let mut tile_value = 1u32;
                    if tile_x == 0 && (!door_left || tile_y != tiles_per_height / 2) {
                        tile_value = 2;
                    }

                    if tile_x == tiles_per_width - 1
                        && (!door_right || tile_y != tiles_per_height / 2)
                    {
                        tile_value = 2;
                    }

                    if tile_y == 0 && (!door_bottom || tile_x != tiles_per_width / 2) {
                        tile_value = 2;
                    }

                    if tile_y == tiles_per_height - 1
                        && (!door_top || tile_x != tiles_per_width / 2)
                    {
                        tile_value = 2;
                    }

                    if tile_x == 10 && tile_y == 6 {
                        if door_up {
                            tile_value = 3;
                        }

                        if door_down {
                            tile_value = 4;
                        }
                    }

                    set_tile_value(
                        &mut game_state.world_arena,
                        tile_map,
                        abs_tile_x,
                        abs_tile_y,
                        abs_tile_z,
                        tile_value,
                    );
                }
            }

            door_left = door_right;
            door_bottom = door_top;

            if created_z_door {
                door_down = !door_down;
                door_up = !door_up;
            } else {
                door_up = false;
                door_down = false;
            }

            door_right = false;
            door_top = false;

            if random_choice == 2 {
                if abs_tile_z == 0 {
                    abs_tile_z = 1;
                } else {
                    abs_tile_z = 0;
                }
            } else if random_choice == 1 {
                screen_x += 1;
            } else {
                screen_y += 1;
            }
        }

        memory.is_initialized = true;
    }

    let game_state = memory.get_game_state();
    let world = &game_state.world;
    let mut tile_map = unsafe { world.as_mut_unchecked().tile_map.as_ref_unchecked() };

    let tile_side_in_pixels = 60;
    let meters_to_pixels = tile_side_in_pixels as f32 / tile_map.tile_side_in_meters;

    let lower_left_x = -tile_side_in_pixels as f32 / 2.;
    let lower_left_y = buffer.height as f32;

    for (controller_index, controller) in inputs.controllers.iter_mut().enumerate() {
        if let Some(controlling_entity) = get_entity(
            game_state,
            game_state.player_index_for_controllers[controller_index],
        ) {
            let mut ddp = V2::default();

            if controller.is_analog {
                ddp = V2 {
                    x: controller.stick_average_x,
                    y: controller.stick_average_y,
                };
            } else {
                if controller.move_up().ended_down {
                    ddp.y = 1.;
                }
                if controller.move_down().ended_down {
                    ddp.y = -1.;
                }
                if controller.move_left().ended_down {
                    ddp.x = -1.;
                }
                if controller.move_right().ended_down {
                    ddp.x = 1.;
                }
            }

            move_player(
                memory.get_game_state(),
                controlling_entity,
                inputs.dt_for_frame,
                ddp,
            );
        } else {
            if controller.start().ended_down {
                let entity_index = add_entity(game_state);
                initialize_player(game_state, entity_index);
                game_state.player_index_for_controllers[controller_index] = entity_index;
            }
        }
    }

    if let Some(camera_following_entity) =
        get_entity(game_state, game_state.camera_following_entity_index)
    {
        let game_state = memory.get_game_state();
        game_state.camera_p.abs_tile_z = camera_following_entity.p.abs_tile_z;

        let diff = subtract(tile_map, camera_following_entity.p, game_state.camera_p);
        if diff.dxy.x > (9. * tile_map.tile_side_in_meters) {
            game_state.camera_p.abs_tile_x += 17;
        }
        if diff.dxy.x < -(9. * tile_map.tile_side_in_meters) {
            game_state.camera_p.abs_tile_x -= 17;
        }
        if diff.dxy.y > (5. * tile_map.tile_side_in_meters) {
            game_state.camera_p.abs_tile_y += 9;
        }
        if diff.dxy.y < -(5. * tile_map.tile_side_in_meters) {
            game_state.camera_p.abs_tile_y -= 9;
        }
    }

    draw_bitmap(buffer, &game_state.back_drop, 0., 0., 0, 0);

    let screen_center_x = 0.5 * buffer.width as f32;
    let screen_center_y = 0.5 * buffer.height as f32;

    for rel_row in -10..10i32 {
        for rel_column in -20..20i32 {
            let column = game_state.camera_p.abs_tile_x as i32 + rel_column;
            let row = game_state.camera_p.abs_tile_y as i32 + rel_row;
            let tile_id = get_tile_value(
                tile_map,
                column as u32,
                row as u32,
                game_state.camera_p.abs_tile_z as u32,
            );

            if tile_id > 1 {
                let mut gray = 0.5;
                if tile_id == 2 {
                    gray = 1.0;
                }

                if tile_id > 2 {
                    gray = 0.25;
                }

                if column == game_state.camera_p.abs_tile_x as i32
                    && row == game_state.camera_p.abs_tile_y as i32
                {
                    gray = 0.
                };

                let tile_side = V2 {
                    x: 0.5 * tile_side_in_pixels as f32,
                    y: 0.5 * tile_side_in_pixels as f32,
                };
                let cen = V2 {
                    x: screen_center_x - meters_to_pixels * game_state.camera_p.offset_.x
                        + rel_column as f32 * tile_side_in_pixels as f32,
                    y: screen_center_y + meters_to_pixels * game_state.camera_p.offset_.y
                        - rel_row as f32 * tile_side_in_pixels as f32,
                };
                let min = cen - 0.9 * tile_side;
                let max = cen + 0.9 * tile_side;
                draw_rectangle(buffer, min, max, gray, gray, gray);
            }
        }
    }

    for entity in &game_state.entities[0..game_state.entity_count] {
        if entity.exists {
            let diff = subtract(tile_map, entity.p, game_state.camera_p);

            let player_r = 1.0;
            let player_g = 1.0;
            let player_b = 0.0;
            let player_ground_point_x = screen_center_x + diff.dxy.x * meters_to_pixels;
            let player_ground_point_y = screen_center_y - diff.dxy.y * meters_to_pixels;
            let player_left_top = V2 {
                x: player_ground_point_x - 0.5 * meters_to_pixels * entity.width,
                y: player_ground_point_y - 0.5 * meters_to_pixels * entity.height,
            };
            let entity_width_height = V2 {
                x: entity.width,
                y: entity.height,
            };
            draw_rectangle(
                buffer,
                player_left_top,
                player_left_top + meters_to_pixels * entity_width_height,
                player_r,
                player_g,
                player_b,
            );

            let hero_bitmap = &game_state.hero_bitmaps[entity.facing_direction as usize];
            draw_bitmap(
                buffer,
                &hero_bitmap.torso,
                player_ground_point_x,
                player_ground_point_y,
                hero_bitmap.align_x,
                hero_bitmap.align_y,
            );
            draw_bitmap(
                buffer,
                &hero_bitmap.cape,
                player_ground_point_x,
                player_ground_point_y,
                hero_bitmap.align_x,
                hero_bitmap.align_y,
            );
            draw_bitmap(
                buffer,
                &hero_bitmap.head,
                player_ground_point_x,
                player_ground_point_y,
                hero_bitmap.align_x,
                hero_bitmap.align_y,
            );
        }
    }
}

#[no_mangle]
extern "C" fn game_get_sound_samples(
    _thread: &ThreadContext,
    memory: &mut GameMemory,
    sound_buffer: &GameSoundBuffer,
) {
    let game_state = memory.get_game_state();
    game_output_sound(game_state, sound_buffer, 480);
}

fn game_output_sound(game_state: &mut GameState, sound_buffer: &GameSoundBuffer, tone_hz: i32) {
    let tone_volume = 3000.;
    let wave_period = sound_buffer.samples_per_second as f32 / tone_hz as f32;

    for sample in sound_buffer
        .samples()
        .chunks_exact_mut(sound_buffer.bytes_per_sample as usize)
    {
        cfg_select! {
            false => {
                let sine_value = game_state.t_sine.sin();
                let sample_value = ((sine_value * tone_volume) as i16).to_be_bytes();
            }
            _ => {
                let sample_value = (0 as i16).to_be_bytes();
            }
        };

        for sample_per_channel in sample.chunks_exact_mut(sample_value.len()) {
            sample_per_channel.copy_from_slice(&sample_value);
        }

        #[cfg(any())]
        {
            game_state.t_sine += 2. * std::f32::consts::PI * 1. / wave_period;
            game_state.t_sine = game_state.t_sine.rem_euclid(2. * std::f32::consts::PI);
        }
    }
}

fn render_weird_gradient(buffer: &mut GameOffscreenBuffer, x_offset: i32, y_offset: i32) {
    let rows = buffer
        .as_slice_mut()
        .chunks_exact_mut(buffer.pitch as usize);
    for (y, row) in rows.enumerate() {
        let pixels = row.chunks_exact_mut(buffer.bytes_per_pixel as usize);
        for (x, pixel) in pixels.enumerate() {
            let blue = (x as i32 + x_offset) & 0xFF;
            let green = (y as i32 + y_offset) & 0xFF;
            pixel.copy_from_slice(&(green << 8 | blue).to_ne_bytes());
        }
    }
}

const _: GameUpdateAndRender = game_update_and_render;
const _: GameGetSoundSample = game_get_sound_samples;
