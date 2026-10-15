#[path = "handmade_platform.rs"]
mod platform;
pub use self::platform::*;

macro_rules! kilobytes {
    ($value:expr) => {
        $value * 1024
    };
}
macro_rules! megabytes {
    ($value:expr) => {
        kilobytes!($value) * 1024
    };
}
macro_rules! gigabytes {
    ($value:expr) => {
        megabytes!($value) * 1024
    };
}
macro_rules! terabytes {
    ($value:expr) => {
        gigabytes!($value) * 1024
    };
}

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

pub struct GameState {
    world_arena: MemoryArena,
    world: *mut World,

    player_p: TileMapPosition,
    pixel_pointer: *mut (),
}

fn draw_rectangle(
    buffer: &mut GameOffscreenBuffer,
    real_min_x: f32,
    real_min_y: f32,
    real_max_x: f32,
    real_max_y: f32,
    r: f32,
    g: f32,
    b: f32,
) {
    let max_x = round_f32_to_i32(real_max_x).min(buffer.width);
    let max_y = round_f32_to_i32(real_max_y).min(buffer.height);
    let min_x = round_f32_to_i32(real_min_x).max(0).min(max_x);
    let min_y = round_f32_to_i32(real_min_y).max(0).min(max_y);

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
}

pub fn debug_load_bmp(
    thread: &ThreadContext,
    read_entire_file: DebugPlatformReadEntireFile,
    file_name: &str,
) -> *mut () {
    let mut result = std::ptr::null_mut();

    if let Some((content, size)) = read_entire_file(thread, file_name) {
        if size != 0 {
            let header = unsafe { content.cast::<BitmapHeader>().as_ref_unchecked() };
            let pixels = content.wrapping_byte_add(header.bitmap_offset as usize);
            result = pixels;
        }
    }

    result
}

#[no_mangle]
extern "C" fn game_update_and_render(
    thread: &ThreadContext,
    memory: &mut GameMemory,
    inputs: &mut GameInput,
    buffer: &mut GameOffscreenBuffer,
) {
    let player_height = 1.4;
    let player_width = 0.75 * player_height;

    if !memory.is_initialized {
        let game_state = memory.get_game_state();

        game_state.pixel_pointer = debug_load_bmp(
            thread,
            memory.read_entire_file_stub.unwrap(),
            "test/test_background.bmp\0",
        );

        game_state.player_p.abs_tile_x = 1;
        game_state.player_p.abs_tile_y = 3;
        game_state.player_p.offset_x = 5.;
        game_state.player_p.offset_y = 5.;

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
        let mut screen_x = 0u32;
        let mut screen_y = 0u32;
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

    for controller in &mut inputs.controllers {
        if controller.is_connected {
            if controller.is_analog {
            } else {
                let mut dplayer_x = 0.;
                let mut dplayer_y = 0.;

                if controller.move_up().ended_down {
                    dplayer_y = 1.;
                }
                if controller.move_down().ended_down {
                    dplayer_y = -1.;
                }
                if controller.move_left().ended_down {
                    dplayer_x = -1.;
                }
                if controller.move_right().ended_down {
                    dplayer_x = 1.;
                }
                let mut player_speed = 2.0;
                if controller.action_up().ended_down {
                    player_speed = 10.0;
                }
                dplayer_x *= player_speed;
                dplayer_y *= player_speed;

                let mut new_player_p = game_state.player_p;
                new_player_p.offset_x += inputs.dt_for_frame * dplayer_x;
                new_player_p.offset_y += inputs.dt_for_frame * dplayer_y;
                new_player_p = recanonicalize_position(tile_map, new_player_p);

                let mut player_left = new_player_p;
                player_left.offset_x -= 0.5 * player_width;
                player_left = recanonicalize_position(tile_map, player_left);

                let mut player_right = new_player_p;
                player_right.offset_x += 0.5 * player_width;
                player_right = recanonicalize_position(tile_map, player_right);

                if is_tile_map_point_empty(tile_map, new_player_p)
                    && is_tile_map_point_empty(tile_map, player_left)
                    && is_tile_map_point_empty(tile_map, player_right)
                {
                    if !are_on_same_tile(&game_state.player_p, &new_player_p) {
                        let new_tile_value = new_player_p.get_tile_value(tile_map);

                        if new_tile_value == 3 {
                            new_player_p.abs_tile_z += 1;
                        } else if new_tile_value == 4 {
                            new_player_p.abs_tile_z -= 1;
                        }
                    }

                    game_state.player_p = new_player_p;
                }
            }
        }
    }

    draw_rectangle(
        buffer,
        0.0,
        0.0,
        buffer.width as f32,
        buffer.height as f32,
        1.0,
        0.0,
        0.1,
    );

    let screen_center_x = 0.5 * buffer.width as f32;
    let screen_center_y = 0.5 * buffer.height as f32;

    for rel_row in -10..10i32 {
        for rel_column in -20..20i32 {
            let column = game_state.player_p.abs_tile_x + rel_column;
            let row = game_state.player_p.abs_tile_y + rel_row;
            let tile_id = get_tile_value(
                tile_map,
                column as u32,
                row as u32,
                game_state.player_p.abs_tile_z as u32,
            );

            if tile_id > 0 {
                let mut gray = 0.5;
                if tile_id == 2 {
                    gray = 1.0;
                }

                if tile_id > 2 {
                    gray = 0.25;
                }

                if column == game_state.player_p.abs_tile_x && row == game_state.player_p.abs_tile_y
                {
                    gray = 0.
                };

                let cen_x = screen_center_x - meters_to_pixels * game_state.player_p.offset_x
                    + rel_column as f32 * tile_side_in_pixels as f32;
                let cen_y = screen_center_y + meters_to_pixels * game_state.player_p.offset_y
                    - rel_row as f32 * tile_side_in_pixels as f32;
                let min_x = cen_x - 0.5 * tile_side_in_pixels as f32;
                let min_y = cen_y - 0.5 * tile_side_in_pixels as f32;
                let max_x = cen_x + 0.5 * tile_side_in_pixels as f32;
                let max_y = cen_y + 0.5 * tile_side_in_pixels as f32;
                draw_rectangle(buffer, min_x, min_y, max_x, max_y, gray, gray, gray);
            }
        }
    }

    let player_r = 1.0;
    let player_g = 1.0;
    let player_b = 0.0;
    let player_left = screen_center_x - 0.5 * meters_to_pixels * player_width;
    let player_top = screen_center_y - meters_to_pixels * player_height;
    draw_rectangle(
        buffer,
        player_left,
        player_top,
        player_left + meters_to_pixels * player_width,
        player_top + meters_to_pixels * player_height,
        player_r,
        player_g,
        player_b,
    );

    #[cfg(any())]
    {
        let source = unsafe {
            std::slice::from_raw_parts(
                game_state.pixel_pointer as *const u8,
                (buffer.width * buffer.pitch) as usize,
            )
        };
        for (dest, source) in buffer.as_slice_mut().iter_mut().zip(source) {
            *dest = *source;
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
        #[cfg(any())]
        let sample_value = {
            let sine_value = game_state.t_sine.sin();
            ((sine_value * tone_volume) as i16).to_be_bytes()
        };
        #[cfg(all())]
        let sample_value = (0 as i16).to_be_bytes();

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
