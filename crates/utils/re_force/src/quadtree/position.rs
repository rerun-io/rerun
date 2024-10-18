pub trait Position {
    fn x(&self) -> f32;
    fn y(&self) -> f32;
}

impl Position for [f32; 2] {
    #[inline(always)]
    fn x(&self) -> f32 {
        self[0]
    }
    #[inline(always)]
    fn y(&self) -> f32 {
        self[1]
    }
}

impl Position for &[f32; 2] {
    #[inline(always)]
    fn x(&self) -> f32 {
        self[0]
    }
    #[inline(always)]
    fn y(&self) -> f32 {
        self[1]
    }
}

impl Position for (f32, f32) {
    #[inline(always)]
    fn x(&self) -> f32 {
        self.0
    }
    #[inline(always)]
    fn y(&self) -> f32 {
        self.1
    }
}

impl Position for &(f32, f32) {
    #[inline(always)]
    fn x(&self) -> f32 {
        self.0
    }
    #[inline(always)]
    fn y(&self) -> f32 {
        self.1
    }
}
