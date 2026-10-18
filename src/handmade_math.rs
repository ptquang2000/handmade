#[derive(Default, Clone, Copy)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl std::ops::Index<usize> for V2 {
    type Output = f32;
    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            _ => panic!("V2 index out of bounds: {}", index),
        }
    }
}

impl std::ops::Mul<V2> for f32 {
    type Output = V2;
    fn mul(self, rhs: V2) -> Self::Output {
        let mut result = Self::Output::default();

        result.x = self * rhs.x;
        result.y = self * rhs.y;

        result
    }
}

impl std::ops::Mul<f32> for V2 {
    type Output = V2;
    fn mul(self, rhs: f32) -> Self::Output {
        let mut result = Self::Output::default();

        result.x = self.x * rhs;
        result.y = self.y * rhs;

        result
    }
}

impl std::ops::MulAssign<f32> for V2 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl std::ops::Neg for V2 {
    type Output = V2;
    fn neg(self) -> Self::Output {
        let mut result = Self::Output::default();

        result.x = -self.x;
        result.y = -self.y;

        result
    }
}

impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, rhs: V2) -> Self::Output {
        let mut result = Self::Output::default();

        result.x = self.x + rhs.x;
        result.y = self.y + rhs.y;

        result
    }
}

impl std::ops::AddAssign for V2 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl std::ops::Sub for V2 {
    type Output = V2;
    fn sub(self, rhs: V2) -> Self::Output {
        let mut result = Self::Output::default();

        result.x = self.x - rhs.x;
        result.y = self.y - rhs.y;

        result
    }
}
