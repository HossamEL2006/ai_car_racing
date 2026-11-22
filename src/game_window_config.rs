pub const CAMERA_WIDTH: u32 = 1200;
pub const CAMERA_HEIGHT: u32 = 900;
pub const SIDE_PANEL_WIDTH: u32 = 400;

pub const SCREEN_WIDTH: u32 = CAMERA_WIDTH + SIDE_PANEL_WIDTH;
pub const SCREEN_HEIGHT: u32 = CAMERA_HEIGHT;

pub const JOYSTICK_POSITION: (i16, i16) = (
    CAMERA_WIDTH as i16 + SIDE_PANEL_WIDTH as i16 / 2,
    CAMERA_HEIGHT as i16 / 3,
);
pub const JOYSTICK_INNER_RADIUS: f32 = 50.;
pub const JOYSTICK_OUTER_RADIUS: f32 = 100.;

// const CIRCUIT_DEBUG: bool = false;
pub const MOUVEMENT_DEBUG: bool = true;

pub const VSYNC: bool = true;
