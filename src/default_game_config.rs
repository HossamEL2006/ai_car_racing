pub const N_ANCHORS: usize = 8;
pub const MEAN_ANCHOR_RADIUS: f32 = 120.;
pub const STDEV_ANCHOR_RADIUS: f32 = 50.;
pub const STDEV_SUBANCHOR_ANGLE_DEV: f32 = (360. / N_ANCHORS as f32) / 3.0;
pub const POINTS_PER_BEZIER_CURVE: usize = 25;
pub const PATH_WIDTH: f32 = 15.0;

pub const CAR_LENGTH: f32 = 5.0; // in meters
pub const CAR_WIDTH: f32 = 2.0; // in meters

pub const LN_100: f32 = 4.60517;
pub const TIME_TO_99_PERC_SPEED: f32 = 5.; // in s
pub const THEORETICAL_MAX_SPEED: f32 = 100.; // in m/s
pub const FRICTION_FACTOR: f32 = LN_100 / TIME_TO_99_PERC_SPEED;
pub const ENGINE_ACCELERATION: f32 = THEORETICAL_MAX_SPEED * FRICTION_FACTOR;

pub const GAME_FPS: f32 = 60.0;
pub const DT: f32 = GAME_FPS.recip();
