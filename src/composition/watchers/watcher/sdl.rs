/// SDL from a user updating the SupergraphConfig directly
#[derive(Debug, Clone)]
pub struct Sdl {
    /// Changed SDL
    sdl: String,
}

impl Sdl {
    pub fn new(sdl: String) -> Self {
        Self { sdl }
    }

    pub fn run(&self) -> String {
        self.sdl.clone()
    }
}
