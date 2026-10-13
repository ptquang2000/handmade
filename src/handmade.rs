include!("handmade_intrinsics.rs");
include!("handmade_tile.rs");

pub mod debug_platform {
    use super::game::Thread;

    pub type ReadEntireFile = fn(thread: &Thread, filename: &str) -> Option<(*mut (), i64)>;
    pub type WriteEntireFile =
        fn(thread: &Thread, filename: &str, memory: *mut (), memory_size: i64) -> bool;
    pub type FreeFileMemory = fn(thread: &Thread, memory: *mut (), size: i64);
}

pub mod game {
    type MemoryIndex = usize;

    use super::debug_platform;
    use super::intrinsics::*;
    use super::tile::*;

    pub struct MemoryArena {
        size: MemoryIndex,
        base: *mut u8,
        used: MemoryIndex,
    }

    pub type UpdateAndRender = extern "C" fn(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    );
    const _: UpdateAndRender = update_and_render;
    pub type GetSoundSample =
        extern "C" fn(thread: &Thread, memory: &mut Memory, sound_buffer: &SoundBuffer);
    const _: GetSoundSample = get_sound_samples;

    #[repr(C)]
    #[derive(Default)]
    pub struct OffscreenBuffer {
        pub memory: *mut u8,
        pub bytes_per_pixel: i32,
        pub width: i32,
        pub height: i32,
        pub pitch: i32,
    }

    impl OffscreenBuffer {
        fn as_slice_mut(&self) -> &mut [u8] {
            let size = self.pitch * self.height;
            unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
        }
    }

    #[derive(Default)]
    pub struct SoundBuffer {
        pub samples_per_second: i32,
        pub bytes_per_sample: i32,
        pub sample_count: i32,
        pub memory: *mut u8,
    }

    impl SoundBuffer {
        fn samples(&self) -> &mut [u8] {
            let size = self.sample_count * self.bytes_per_sample;
            unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
        }
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct ButtonState {
        pub half_transition_count: i32,
        pub ended_down: bool,
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct ControllerInput {
        pub is_connected: bool,
        pub is_analog: bool,
        pub stick_average_x: f32,
        pub stick_average_y: f32,
        pub buttons: [ButtonState; 12],
    }

    impl ControllerInput {
        const MOVE_UP: usize = 0;
        const MOVE_DOWN: usize = 1;
        const MOVE_LEFT: usize = 2;
        const MOVE_RIGHT: usize = 3;
        const ACTION_UP: usize = 4;
        const ACTION_DOWN: usize = 5;
        const ACTION_LEFT: usize = 6;
        const ACTION_RIGHT: usize = 7;
        const LEFT_SHOULDER: usize = 8;
        const RIGHT_SHOULDER: usize = 9;
        const START: usize = 10;
        const BACK: usize = 11;

        pub fn move_up(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_UP]
        }
        pub fn move_down(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_DOWN]
        }
        pub fn move_left(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_LEFT]
        }
        pub fn move_right(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::MOVE_RIGHT]
        }
        pub fn action_up(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_UP]
        }
        pub fn action_down(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_DOWN]
        }
        pub fn action_left(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_LEFT]
        }
        pub fn action_right(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::ACTION_RIGHT]
        }
        pub fn left_shoulder(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::LEFT_SHOULDER]
        }
        pub fn right_shoulder(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::RIGHT_SHOULDER]
        }
        pub fn start(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::START]
        }
        pub fn back(&mut self) -> &mut ButtonState {
            &mut self.buttons[Self::BACK]
        }
    }

    #[repr(C)]
    #[derive(Default, Copy, Clone)]
    pub struct Input {
        pub mouse_buttons: [ButtonState; 5],
        pub mouse_x: i32,
        pub mouse_y: i32,
        pub mouse_z: i32,

        pub dt_for_frame: f32,

        pub controllers: [ControllerInput; 5],
    }

    #[derive(Default)]
    pub struct Memory {
        pub is_initialized: bool,

        pub permanent_storage_size: usize,
        pub permanent_storage_memory: *mut u8,

        pub transient_storage_size: usize,
        pub transient_storage_memory: *mut u8,

        pub read_entire_file_stub: Option<debug_platform::ReadEntireFile>,
        pub write_entire_file_stub: Option<debug_platform::WriteEntireFile>,
        pub free_file_memory_stub: Option<debug_platform::FreeFileMemory>,
    }

    impl Memory {
        fn get_game_state(&self) -> &mut State {
            assert!(std::mem::size_of::<State>() <= self.transient_storage_size as usize);
            unsafe { &mut *(self.permanent_storage_memory as *mut State) }
        }

        fn read_entire_file(&self, thread: &Thread, filename: &str) -> Option<(*mut (), i64)> {
            if let Some(func) = self.read_entire_file_stub {
                func(thread, filename)
            } else {
                None
            }
        }

        fn write_entire_file(
            &self,
            thread: &Thread,
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

        fn free_file_memory(&self, thread: &Thread, memory: *mut (), size: i64) {
            if let Some(func) = self.free_file_memory_stub {
                func(thread, memory, size)
            }
        }
    }

    #[derive(Default)]
    pub struct Thread {
        _placeholder: i32,
    }

    pub struct State {
        world_arena: MemoryArena,
        world: *mut World,

        player_p: TileMapPosition,
    }

    #[derive(Default)]
    struct World {
        tile_map: *mut TileMap,
    }

    #[derive(Default, Clone, Copy)]
    struct RawPosition {
        tile_map_x: i32,
        tile_map_y: i32,

        x: f32,
        y: f32,
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
    fn push_size_(arena: &mut MemoryArena, size: MemoryIndex) -> *mut u8 {
        debug_assert!(arena.used + size <= arena.size);
        let result = arena.base.wrapping_add(arena.used);
        arena.used += size;
        result
    }

    #[no_mangle]
    extern "C" fn update_and_render(
        thread: &Thread,
        memory: &mut Memory,
        inputs: &mut Input,
        buffer: &mut OffscreenBuffer,
    ) {
        let player_height = 1.4;
        let player_width = 0.75 * player_height;

        if !memory.is_initialized {
            let game_state = memory.get_game_state();

            game_state.player_p.abs_tile_x = 1;
            game_state.player_p.abs_tile_y = 3;
            game_state.player_p.tile_rel_x = 5.;
            game_state.player_p.tile_rel_y = 5.;

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
            tile_map.tile_chunks = push_array!(
                &mut game_state.world_arena,
                tile_map.tile_chunk_count_x * tile_map.tile_chunk_count_y,
                TileChunk
            );
            let tile_chunks = unsafe {
                std::slice::from_raw_parts_mut(
                    tile_map.tile_chunks,
                    (tile_map.tile_chunk_count_x * tile_map.tile_chunk_count_y) as usize,
                )
            };
            for tile_chunk in tile_chunks {
                tile_chunk.tiles = push_array!(
                    &mut game_state.world_arena,
                    tile_map.chunk_dim * tile_map.chunk_dim,
                    u32
                );
            }

            tile_map.tile_side_in_meters = 1.4;
            tile_map.tile_side_in_pixels = 60;
            tile_map.meters_to_pixels =
                tile_map.tile_side_in_pixels as f32 / tile_map.tile_side_in_meters;

            let lower_left_x = -tile_map.tile_side_in_pixels as f32 / 2.;
            let lower_left_y = buffer.height as f32;

            let tiles_per_width = 17u32;
            let tiles_per_height = 9u32;
            for screen_y in 0..32u32 {
                for screen_x in 0..32u32 {
                    for tile_y in 0..tiles_per_height {
                        for tile_x in 0..tiles_per_width {
                            let abs_tile_x = screen_x * tiles_per_width + tile_x;
                            let abs_tile_y = screen_y * tiles_per_height + tile_y;

                            set_tile_value(
                                &game_state.world_arena,
                                tile_map,
                                abs_tile_x,
                                abs_tile_y,
                                ((tile_x == tile_y) && (tile_y % 2 == 1)) as u32,
                            );
                        }
                    }
                }
            }

            memory.is_initialized = true;
        }

        let game_state = memory.get_game_state();
        let world = &game_state.world;
        let mut tile_map = unsafe { world.as_mut_unchecked().tile_map.as_ref_unchecked() };

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
                    new_player_p.tile_rel_x += inputs.dt_for_frame * dplayer_x;
                    new_player_p.tile_rel_y += inputs.dt_for_frame * dplayer_y;
                    new_player_p = recanonicalize_position(tile_map, new_player_p);

                    let mut player_left = new_player_p;
                    player_left.tile_rel_x -= 0.5 * player_width;
                    player_left = recanonicalize_position(tile_map, player_left);

                    let mut player_right = new_player_p;
                    player_right.tile_rel_x += 0.5 * player_width;
                    player_right = recanonicalize_position(tile_map, player_right);

                    if is_tile_map_point_empty(tile_map, new_player_p)
                        && is_tile_map_point_empty(tile_map, player_left)
                        && is_tile_map_point_empty(tile_map, player_right)
                    {
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
                let tile_id = get_tile_value(tile_map, column as u32, row as u32);
                let mut gray = 0.5;
                if tile_id == 1 {
                    gray = 1.0;
                }

                if column == game_state.player_p.abs_tile_x && row == game_state.player_p.abs_tile_y
                {
                    gray = 0.
                };

                let cen_x = screen_center_x
                    - tile_map.meters_to_pixels * game_state.player_p.tile_rel_x
                    + rel_column as f32 * tile_map.tile_side_in_pixels as f32;
                let cen_y = screen_center_y
                    + tile_map.meters_to_pixels * game_state.player_p.tile_rel_y
                    - rel_row as f32 * tile_map.tile_side_in_pixels as f32;
                let min_x = cen_x - 0.5 * tile_map.tile_side_in_pixels as f32;
                let min_y = cen_y - 0.5 * tile_map.tile_side_in_pixels as f32;
                let max_x = cen_x + 0.5 * tile_map.tile_side_in_pixels as f32;
                let max_y = cen_y + 0.5 * tile_map.tile_side_in_pixels as f32;
                draw_rectangle(buffer, min_x, min_y, max_x, max_y, gray, gray, gray);
            }
        }

        let player_r = 1.0;
        let player_g = 1.0;
        let player_b = 0.0;
        let player_left = screen_center_x - 0.5 * tile_map.meters_to_pixels * player_width;
        let player_top = screen_center_y - tile_map.meters_to_pixels * player_height;
        draw_rectangle(
            buffer,
            player_left,
            player_top,
            player_left + tile_map.meters_to_pixels * player_width,
            player_top + tile_map.meters_to_pixels * player_height,
            player_r,
            player_g,
            player_b,
        );
    }

    #[no_mangle]
    extern "C" fn get_sound_samples(
        _thread: &Thread,
        memory: &mut Memory,
        sound_buffer: &SoundBuffer,
    ) {
        let game_state = memory.get_game_state();
        output_sound(game_state, sound_buffer, 480);
    }

    fn output_sound(game_state: &mut State, sound_buffer: &SoundBuffer, tone_hz: i32) {
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

    fn draw_rectangle(
        buffer: &mut OffscreenBuffer,
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

    fn render_weird_gradient(buffer: &mut OffscreenBuffer, x_offset: i32, y_offset: i32) {
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
}
