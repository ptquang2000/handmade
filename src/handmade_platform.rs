use super::*;

pub type MemoryIndex = usize;

pub trait ArrayLength {
    const LEN: usize;
}
impl<T, const N: usize> ArrayLength for [T; N] {
    const LEN: usize = N;
}
macro_rules! array_len {
    ($type:ty) => {
        <$type as ArrayLength>::LEN
    };
}
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

#[derive(Default)]
pub struct ThreadContext {
    _placeholder: i32,
}

pub type DebugPlatformReadEntireFile =
    fn(thread: &ThreadContext, filename: &str) -> Option<(*mut (), i64)>;
pub type DebugPlatformWriteEntireFile =
    fn(thread: &ThreadContext, filename: &str, memory: *mut (), memory_size: i64) -> bool;
pub type DebugPlatformFreeFileMemory = fn(thread: &ThreadContext, memory: *mut (), size: i64);

#[repr(C)]
#[derive(Default)]
pub struct GameOffscreenBuffer {
    pub memory: *mut u8,
    pub bytes_per_pixel: i32,
    pub width: i32,
    pub height: i32,
    pub pitch: i32,
}

impl GameOffscreenBuffer {
    pub fn as_slice_mut(&self) -> &mut [u8] {
        let size = self.pitch * self.height;
        unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
    }
}

#[derive(Default)]
pub struct GameSoundBuffer {
    pub samples_per_second: i32,
    pub bytes_per_sample: i32,
    pub sample_count: i32,
    pub memory: *mut u8,
}

impl GameSoundBuffer {
    pub fn samples(&self) -> &mut [u8] {
        let size = self.sample_count * self.bytes_per_sample;
        unsafe { std::slice::from_raw_parts_mut(self.memory, size as usize) }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct GameButtonState {
    pub half_transition_count: i32,
    pub ended_down: bool,
}

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct GameControllerInput {
    pub is_connected: bool,
    pub is_analog: bool,
    pub stick_average_x: f32,
    pub stick_average_y: f32,
    pub buttons: [GameButtonState; 12],
}

impl GameControllerInput {
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

    pub fn move_up(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::MOVE_UP]
    }
    pub fn move_down(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::MOVE_DOWN]
    }
    pub fn move_left(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::MOVE_LEFT]
    }
    pub fn move_right(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::MOVE_RIGHT]
    }
    pub fn action_up(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::ACTION_UP]
    }
    pub fn action_down(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::ACTION_DOWN]
    }
    pub fn action_left(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::ACTION_LEFT]
    }
    pub fn action_right(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::ACTION_RIGHT]
    }
    pub fn left_shoulder(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::LEFT_SHOULDER]
    }
    pub fn right_shoulder(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::RIGHT_SHOULDER]
    }
    pub fn start(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::START]
    }
    pub fn back(&mut self) -> &mut GameButtonState {
        &mut self.buttons[Self::BACK]
    }
}

pub type Controllers = [GameControllerInput; 5];
#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct GameInput {
    pub mouse_buttons: [GameButtonState; 5],
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_z: i32,

    pub dt_for_frame: f32,

    pub controllers: Controllers,
}

#[derive(Default)]
pub struct GameMemory {
    pub is_initialized: bool,

    pub permanent_storage_size: usize,
    pub permanent_storage_memory: *mut u8,

    pub transient_storage_size: usize,
    pub transient_storage_memory: *mut u8,

    pub read_entire_file_stub: Option<DebugPlatformReadEntireFile>,
    pub write_entire_file_stub: Option<DebugPlatformWriteEntireFile>,
    pub free_file_memory_stub: Option<DebugPlatformFreeFileMemory>,
}

pub type GameUpdateAndRender = extern "C" fn(
    thread: &ThreadContext,
    memory: &mut GameMemory,
    inputs: &mut GameInput,
    buffer: &mut GameOffscreenBuffer,
);
pub type GameGetSoundSample =
    extern "C" fn(thread: &ThreadContext, memory: &mut GameMemory, sound_buffer: &GameSoundBuffer);
