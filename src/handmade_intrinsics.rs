pub mod intrinsics {
    pub fn round_f32_to_i32(float32: f32) -> i32 {
        let result = (float32 + 0.5) as i32;
        result
    }

    pub fn round_f32_to_u32(float32: f32) -> u32 {
        let result = (float32 + 0.5) as u32;
        result
    }

    pub fn floor_f32_to_i32(float32: f32) -> i32 {
        let result = float32.floor() as i32;
        result
    }

    pub fn truncate_f32_to_i32(float32: f32) -> i32 {
        let result = float32 as i32;
        result
    }

    pub fn sin(angle: f32) -> f32 {
        let result = angle.sin();
        result
    }

    pub fn cos(angle: f32) -> f32 {
        let result = angle.cos();
        result
    }

    pub fn atan2(y: f32, x: f32) -> f32 {
        let result = y.atan2(x);
        result
    }
}
