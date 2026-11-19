mod game;
mod game_graphic;
mod screen_graphic;
mod texture_cache;
mod utils;
mod world_graphic;
mod world_objects;

use crate::{
    game::Game,
    screen_graphic::{
        CacheableScreenGraphic, CircleGUI, DiscGUI, SizedScreenGraphic, TextFont, TextGUI,
    },
    texture_cache::{HashableGraphicProps, TextureCache},
    world_graphic::CacheableWorldGraphic,
};
use core::f32;
use sdl3::{
    EventPump,
    event::Event,
    joystick::Joystick,
    keyboard::{KeyboardState, Keycode, Scancode},
    pixels::Color,
    render::{self, Canvas, RenderTarget},
    ttf::{self},
    video::{Window, WindowContext},
};
use std::{
    collections::VecDeque,
    time::{self, Duration},
};
use world_objects::*;

struct SideJoystickGUI {
    position: (i16, i16),
    is_clicked: bool,
    direction: WorldVector,
    acceleration_key: Scancode,
    acceleration_pressed: bool,
    inner_radius: f32,
    outer_radius: f32,
}

impl SideJoystickGUI {
    fn new(
        position: (i16, i16),
        acceleration_key: Scancode,
        inner_radius: f32,
        outer_radius: f32,
    ) -> Self {
        Self {
            position,
            is_clicked: false,
            direction: WorldVector::zero(),
            acceleration_key,
            acceleration_pressed: false,
            inner_radius,
            outer_radius,
        }
    }

    fn update(&mut self, event_pump: &EventPump) {
        self.acceleration_pressed = event_pump
            .keyboard_state()
            .is_scancode_pressed(self.acceleration_key);

        let mouse_state = event_pump.mouse_state();
        let mouse_position = (mouse_state.x(), mouse_state.y());
        let mouse_down = mouse_state.left();

        if mouse_down {
            let dist_to_joystick: f32 = ((mouse_position.0 - self.position.0 as f32).powi(2)
                + (mouse_position.1 - self.position.1 as f32).powi(2))
            .sqrt();

            if dist_to_joystick < self.inner_radius {
                self.is_clicked = true;
            }
        } else {
            self.is_clicked = false;
        }

        if self.is_clicked {
            let mouse_x: f32 = mouse_position.0;
            let mouse_y: f32 = mouse_position.1;
            let dx: f32 = mouse_x - self.position.0 as f32;
            let dy: f32 = mouse_y - self.position.1 as f32;
            let max_reach: f32 = self.outer_radius - 0.8 * self.inner_radius;
            self.direction = (1. / max_reach) * WorldVector::new(Meter::new(dx), -Meter::new(dy));
            if self.direction.norm().value() > 1.0 {
                self.direction = self.direction.normalized();
            }
        } else {
            self.direction = WorldVector::zero();
        }
    }

    fn render<'tc, T: RenderTarget, U>(
        &self,
        canvas: &mut Canvas<T>,
        texture_creator: &'tc render::TextureCreator<U>,
        texture_cache: &mut TextureCache<'tc, HashableGraphicProps>,
    ) -> Result<(), ()> {
        let (center_x, center_y) = self.position;
        CircleGUI::new(self.outer_radius, Color::RGB(200, 200, 200))
            .cached_render_onto(
                canvas,
                texture_creator,
                texture_cache,
                (
                    center_x as i32 - self.outer_radius as i32,
                    center_y as i32 - self.outer_radius as i32,
                ),
            )
            .map_err(|_| ())?;

        let max_reach = self.outer_radius - 0.8 * self.inner_radius;
        let pos_x: f32 = center_x as f32 + self.direction.x().value() * max_reach;
        let pos_y: f32 = center_y as f32 - self.direction.y().value() * max_reach;

        DiscGUI::new(
            self.inner_radius,
            if self.is_clicked {
                Color::RGB(125, 125, 125)
            } else {
                Color::RGB(175, 175, 175)
            },
        )
        .cached_render_onto(
            canvas,
            texture_creator,
            texture_cache,
            (
                pos_x as i32 - self.inner_radius as i32,
                pos_y as i32 - self.inner_radius as i32,
            ),
        )
        .map_err(|_| ())?;
        Ok(())
    }
}

struct CenteredJoystick {
    screen_center_pos: (f32, f32),
    button: Scancode,
    direction: WorldVector,
    acceleration_pressed: bool,
}

impl CenteredJoystick {
    fn new(screen_center_pos: (f32, f32), button: Scancode) -> Self {
        Self {
            screen_center_pos,
            button,
            direction: WorldVector::zero(),
            acceleration_pressed: false,
        }
    }

    fn update(&mut self, event_pump: &EventPump) {
        let keyboard_state = KeyboardState::new(event_pump);
        let button_pressed = keyboard_state.is_scancode_pressed(self.button);
        let mouse_pos = (event_pump.mouse_state().x(), event_pump.mouse_state().y());
        let direction = WorldVector::new(
            Meter::new(mouse_pos.0 - self.screen_center_pos.0),
            Meter::new(self.screen_center_pos.1 - mouse_pos.1),
        );
        self.direction = direction.normalized();
        self.acceleration_pressed = button_pressed;
    }
}

enum PlayerControl {
    Gamepad(Joystick),
    SideJoystick(SideJoystickGUI),
    CenteredJoystick(CenteredJoystick),
    RustFunction,
    ExternalFunction,
    None,
}

struct PlayerProfile {
    name: String,
    control: PlayerControl,
}

// impl Game {
//     fn get_player_decision(player_control: &PlayerControl) -> PlayerDecision {
//         match player_control {
//             PlayerControl::Gamepad(joystick) => {
//                 let axis_0: f32 = joystick.axis(0).unwrap_or(0) as f32 / 32768.0;
//                 let axis_1: f32 = joystick.axis(1).unwrap_or(0) as f32 / 32768.0;
//                 let joystick_direction = WorldVector::new(Meter::new(axis_0), -Meter::new(axis_1));
//                 let acceleration_pressed = joystick.button(0).unwrap_or(false);
//                 PlayerDecision {
//                     direction: joystick_direction,
//                     accelerate: acceleration_pressed,
//                 }
//             }
//             PlayerControl::SideJoystick(joystick) => PlayerDecision {
//                 direction: joystick.direction.normalized(),
//                 accelerate: joystick.acceleration_pressed,
//             },
//             PlayerControl::CenteredJoystick(centered_joystick) => PlayerDecision {
//                 direction: centered_joystick.direction.normalized(),
//                 accelerate: centered_joystick.acceleration_pressed,
//             },
//             PlayerControl::RustFunction(_) => todo!(),
//             PlayerControl::ExternalFunction => todo!(),
//             PlayerControl::None => PlayerDecision {
//                 direction: WorldVector::zero(),
//                 accelerate: false,
//             },
//         }
//     }
// }

const CAMERA_WIDTH: u32 = 1200;
const CAMERA_HEIGHT: u32 = 900;
const SIDE_PANEL_WIDTH: u32 = 400;

const SCREEN_WIDTH: u32 = CAMERA_WIDTH + SIDE_PANEL_WIDTH;
const SCREEN_HEIGHT: u32 = CAMERA_HEIGHT;

const JOYSTICK_POSITION: (i16, i16) = (
    CAMERA_WIDTH as i16 + SIDE_PANEL_WIDTH as i16 / 2,
    CAMERA_HEIGHT as i16 / 3,
);
const JOYSTICK_INNER_RADIUS: f32 = 50.;
const JOYSTICK_OUTER_RADIUS: f32 = 100.;

const CIRCUIT_DEBUG: bool = false;
const MOUVEMENT_DEBUG: bool = true;
const DEBUG_MAX_FPS: bool = false;
const REGULAR_LOG_FPS: bool = true;
const REGULAR_LOG_PERF: bool = true;
const REGULARITY_INTERVAL: u32 = 60 * 3;
const LOG_PERF_ON_LAG: bool = true;

const VSYNC: bool = true && !DEBUG_MAX_FPS;

const DEBUG_PERFORMANCE_LABELS: [&str; 11] = [
    "             Input Handeling",
    "                   Car Logic",
    "             Screen Clearing",
    "     Enclosed Area Rendering",
    "      Circuit Path Rendering",
    "Circuit Generation Debugging",
    "               Car Rendering",
    "      Car Movement Debugging",
    "              Text Rendering",
    "        Side Panel Rendering",
    "               Screen Update",
];

const GAME_FPS: f32 = 60.0;
const DT: f32 = GAME_FPS.recip();

pub fn main() {
    // Initialize SDL
    if VSYNC {
        sdl3::hint::set("SDL_RENDER_VSYNC", "1");
    }
    let sdl_context: sdl3::Sdl = sdl3::init().unwrap();
    let video_subsystem: sdl3::VideoSubsystem = sdl_context.video().unwrap();

    // Create a window
    let window: Window = video_subsystem
        .window("Car Racing Game", SCREEN_WIDTH, SCREEN_HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    // Transform the window into a canvas
    let mut canvas: Canvas<Window> = window.into_canvas();

    // Initialize a texture creator
    let texture_creator: render::TextureCreator<WindowContext> = canvas.texture_creator();

    // Initalize connection with a controller joystick
    let mut joystick: Option<Joystick> = None;
    if let Ok(joystick_subsystem) = sdl_context.joystick() {
        println!("Joystick subsystem initialized");
        joystick_subsystem.set_joystick_events_enabled(true);
        if let Some(fst_joystick_id) = joystick_subsystem
            .joysticks()
            .ok()
            .and_then(|joysticks| joysticks.first().cloned())
        {
            joystick = joystick_subsystem
                .open(fst_joystick_id)
                .ok()
                .filter(|js: &Joystick| js.num_axes() >= 2);
        }
    }

    // Initialize the Game
    let control = match joystick {
        Some(js) => PlayerControl::Gamepad(js),
        None => PlayerControl::CenteredJoystick(CenteredJoystick::new(
            (CAMERA_WIDTH as f32 / 2., CAMERA_HEIGHT as f32 / 2.),
            Scancode::Space,
        )),
        // None => PlayerControl::SideJoystick(SideJoystickGUI::new(
        //     JOYSTICK_POSITION,
        //     Scancode::Space,
        //     JOYSTICK_INNER_RADIUS,
        //     JOYSTICK_OUTER_RADIUS,
        // )),
    };

    let mut game: Game = Game::init_game(1, None, DT);

    // Initialize the cameras
    let mut car_camera: Camera = Camera::new(
        CAMERA_WIDTH,
        CAMERA_HEIGHT,
        game.players()[0].car().coords,
        (CAMERA_WIDTH as f32 / (game.players()[0].car().length.value() / 0.04)).round() as u32,
        // car length is 4% of the camera width
    );

    let followed_car_index: usize = 0;

    let fix_camera: Camera = Camera::new(CAMERA_WIDTH, CAMERA_HEIGHT, WorldCoords::origin(), 2);

    let mut current_camera: usize = 0; // 0 for car_camera, 1 for fix_camera

    // Initialize Font
    let ttf_context: ttf::Sdl3TtfContext = ttf::init().unwrap();
    let text_font: TextFont = TextFont::new(
        ttf_context
            .load_font("assets/Basic-Regular.ttf", 25.)
            .unwrap(),
        "Basic-Regular",
    );

    // Initialize the texture caching system
    // TODO: Create a rendering context with texture_cache as a potential struct field
    //       this could include the texture creator, can be passed as one object to rendering
    //       functions rether than always including both the texture creator and the cache
    //       separately. And it could also remove the need to alway have a visible variable of type
    //       Option<TextureCache> circulating in the main function.
    let mut texture_cache: TextureCache<HashableGraphicProps> = TextureCache::new();

    // Render text textures
    let hold_space_text = TextGUI::new(
        "PRESS ENTER TO TOGGLE CAMERAS",
        &text_font,
        Color::RGB(255, 255, 255),
    );

    // TODO : Hande control differently
    let js_name_str = if let PlayerControl::Gamepad(js) = todo!() {
        format!("Controller: {}", js.name())
    } else {
        "No controller detected at startup".to_string()
    };

    let js_name_text = TextGUI::new(&js_name_str, &text_font, Color::RGB(255, 255, 255));

    let seed_text = TextGUI::new(
        &format!("SEED: {}", game.seed()),
        &text_font,
        Color::RGB(255, 255, 255),
    );

    // Initialize an FPS sample queue to monitor average FPS across 3 seconds
    let mut fps_sample: VecDeque<f32> = VecDeque::with_capacity(3 * GAME_FPS as usize);
    for _ in 0..(3 * GAME_FPS as usize) {
        fps_sample.push_back(GAME_FPS);
    }

    // Initialize the event pump for handling SDL events
    let mut event_pump: sdl3::EventPump = sdl_context.event_pump().unwrap();

    'running: loop {
        let time: time::Instant = time::Instant::now();
        let mut timestamps: Vec<u128> = Vec::new();

        // Read the events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Return),
                    ..
                } => {
                    current_camera = 1 - current_camera;
                }
                _ => (),
            }
        }

        for player in game.mut_players() {
            // TODO : Handle control differently
            // let player_control = &mut player.profile.control;
            // if let PlayerControl::SideJoystick(joystick) = player_control {
            //     joystick.update(&event_pump);
            // } else if let PlayerControl::CenteredJoystick(centered_joystick) = player_control {
            //     centered_joystick.update(&event_pump);
            // }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        game.update();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Update cameras
        car_camera.set_coords(game.players()[followed_car_index].car().coords);

        // Determine which camera to use for rendering
        let current_camera: &Camera = if current_camera == 0 {
            &car_camera
        } else {
            &fix_camera
        };

        // Start rendering
        let mut failed_rendering: bool = false;

        // Fill the entire screen with dark green
        canvas.set_draw_color(Color::RGB(25, 50, 25));
        canvas.clear();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Draw the circuit
        failed_rendering |= game
            .track()
            .inner_border()
            .as_graphic(true, Color::RGB(10, 70, 10))
            .cached_render_onto(
                current_camera,
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
            )
            .map_err(|e| eprintln!("Failed to render the enclosed area: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        failed_rendering |= game
            .track()
            .circuit_polygon()
            .as_graphic(true, Color::RGB(80, 80, 80))
            .cached_render_onto(
                current_camera,
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
            )
            .map_err(|e| eprintln!("Failed to render the circuit polygon: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        if CIRCUIT_DEBUG {
            // failed_rendering |= game
            //     .track
            //     .bezier_path
            //     .render(
            //         current_camera,
            //         &mut canvas,
            //         &texture_creator,
            //         &mut texture_cache,
            //     )
            //     .map_err(|e| eprintln!("Failed to render the path: {:?}", e))
            //     .is_err();
            // failed_rendering |= game
            //     .track
            //     .bezier_path
            //     .render_debug_anchors(current_camera, &mut canvas)
            //     .map_err(|e| eprintln!("Failed to render the path debug anchors: {:?}", e))
            //     .is_err();
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Render the car
        for player in game.players() {
            failed_rendering |= &player
                .car()
                .pretty_render(current_camera, &mut canvas)
                .map_err(|e| eprintln!("Failed to render the car: {:?}", e))
                .is_err();
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Debug car vectors
        if MOUVEMENT_DEBUG {
            // for player in &game.players {
            //     failed_rendering |= &player
            //         .car
            //         .render_debug_vectors(
            //             current_camera,
            //             &mut canvas,
            //             &text_font,
            //             &texture_creator,
            //             &mut texture_cache,
            //         )
            //         .map_err(|e| eprintln!("Failed to render the car debug vectors: {:?}", e))
            //         .is_err();
            // }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        failed_rendering |= hold_space_text
            .cached_render_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                (
                    10,
                    SCREEN_HEIGHT as i32 - hold_space_text.height() as i32 - 10,
                ),
            )
            .map_err(|e| eprintln!("Failed to render the hold space text: {:?}", e))
            .is_err();

        failed_rendering |= js_name_text
            .cached_render_onto(
                &mut canvas,
                &texture_creator,
                &mut texture_cache,
                (SCREEN_WIDTH as i32 - js_name_text.width() as i32 - 10, 10),
            )
            .map_err(|e| eprintln!("Failed to render the joystick name text: {:?}", e))
            .is_err();

        failed_rendering |= seed_text
            .cached_render_onto(&mut canvas, &texture_creator, &mut texture_cache, (10, 10))
            .map_err(|e| eprintln!("Failed to render the seed text: {:?}", e))
            .is_err();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Draw a seperating line between the game area and the side panel
        canvas.set_draw_color(Color::RGB(100, 100, 100));
        let (x1, y1) = (CAMERA_WIDTH, 0u32);
        let (x2, y2) = (CAMERA_WIDTH, CAMERA_HEIGHT);
        failed_rendering |= canvas
            .draw_line((x1 as f32, y1 as f32), (x2 as f32, y2 as f32))
            .map_err(|e| eprintln!("Failed to draw the separating line: {:?}", e))
            .is_err();

        // Render the joystick
        for player in game.players() {
            // TODO : Handle control differently
            // if let PlayerControl::SideJoystick(js) = &player.profile.control {
            //     failed_rendering |= js
            //         .render(&mut canvas, &texture_creator, &mut texture_cache)
            //         .map_err(|e| eprintln!("Failed to render the joystick: {:?}", e))
            //         .is_err();
            // }
        }

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Apply the changes on the screen
        canvas.present();

        if REGULAR_LOG_PERF {
            timestamps.push(time.elapsed().as_micros());
        }

        // Detect failed rendering attempts
        if failed_rendering {
            eprintln!(
                "Warning: Failed rendering one or more elements on frame {}.",
                game.game_tick()
            );
        }

        // Sleep to control the FPS
        let mut slept: bool = false;
        if !DEBUG_MAX_FPS {
            let frame_duration: Duration =
                Duration::new(0, (1_000_000_000f32 / GAME_FPS).round() as u32);
            if let Some(remaining_time) = frame_duration.checked_sub(time.elapsed()) {
                std::thread::sleep(remaining_time);
                slept = true;
            }
        }

        // Compute the average FPS
        fps_sample.push_back(1.0 / time.elapsed().as_secs_f32());
        fps_sample.pop_front();
        let average_fps: f32 = fps_sample.iter().sum::<f32>() / fps_sample.len() as f32;
        let mut log_fps: bool =
            REGULAR_LOG_FPS && game.game_tick().is_multiple_of(REGULARITY_INTERVAL);
        let mut log_perf: bool =
            REGULAR_LOG_PERF && game.game_tick().is_multiple_of(REGULARITY_INTERVAL);

        // If average frame rate drops below 80% of the frame target, log a warning
        if average_fps < 0.8 * GAME_FPS {
            eprintln!(
                "Warning: Average FPS dropped below 80% of the target on frame {}.",
                game.game_tick()
            );
            log_fps = true;
            LOG_PERF_ON_LAG.then(|| log_perf = true);
        }

        if log_fps || log_perf {
            println!("Frame {}:", game.game_tick());
        }

        if log_fps {
            println!("Average FPS: {}", average_fps);
        }

        if log_perf {
            if timestamps.len() != DEBUG_PERFORMANCE_LABELS.len() {
                eprintln!("Warning: Mismatch between timestamps and debug performance labels.");
            }

            let frame_duration: u128 = *timestamps.last().unwrap();
            println!(
                "Frame duration (sleep excluded): {} µs -> {:.2}% of frame target",
                frame_duration,
                100.0 * frame_duration as f32 / (1_000_000.0 / GAME_FPS)
            );
            if slept {
                println!("The game slept this frame.");
            }

            println!("Detailed performance breakdown:");

            let mut last_timestamp: u128 = 0;
            for (i, timestamp) in timestamps.iter().enumerate() {
                println!(
                    "{}: {} µs -> {:.1}% of frame target",
                    DEBUG_PERFORMANCE_LABELS[i],
                    timestamp - last_timestamp,
                    100.0 * (timestamp - last_timestamp) as f32 / (1_000_000.0 / GAME_FPS)
                );
                last_timestamp = *timestamp;
            }
        }

        if log_fps || log_perf {
            println!();
        }

        // println!("{}", texture_cache.len());
    }
}
