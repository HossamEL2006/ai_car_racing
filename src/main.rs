use core::f32;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::{Point, Rect};
use sdl3::render::{Canvas, FPoint};
use sdl3::video::Window;
use std::time::Duration;
use std::{iter, ops};

struct Meter(f32);

struct Coords {
    x: Meter,
    y: Meter,
}

struct Vector {
    x: Meter,
    y: Meter,
}

impl ops::Sub for &Coords {
    type Output = Vector;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector {
            x: Meter(self.x.0 - rhs.x.0),
            y: Meter(self.y.0 - rhs.y.0),
        }
    }
}

impl Coords {
    fn init(x: f32, y: f32) -> Coords {
        Coords {
            x: Meter(x),
            y: Meter(y),
        }
    }

    fn as_fpoint(&self, camera: &Camera) -> FPoint {
        let diff_from_camera_center: Vector = self - &camera.coords;
        FPoint::new(
            (camera.width / 2) as f32 + (camera.px_per_meter as f32) * diff_from_camera_center.x.0,
            (camera.height / 2) as f32 + (camera.px_per_meter as f32) * diff_from_camera_center.y.0,
        )
    }

    fn as_point(&self, camera: &Camera) -> Point {
        let diff_from_camera_center: Vector = self - &camera.coords;
        Point::new(
            (camera.width / 2) as i32
                + ((camera.px_per_meter as f32) * diff_from_camera_center.x.0).round() as i32,
            (camera.height / 2) as i32
                + ((camera.px_per_meter as f32) * diff_from_camera_center.y.0).round() as i32,
        )
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        canvas.draw_rect(Rect::from_center(self.as_point(camera), 5, 5))
    }
}

struct Camera {
    width: u32,
    height: u32,
    coords: Coords,
    px_per_meter: u32,
}

fn circular_points(n_points: u32, radius: f32) -> Vec<Coords> {
    (0..n_points)
        .map(|idx: u32| {
            Coords::init(
                radius * (idx as f32 * f32::consts::TAU / n_points as f32).cos(),
                radius * (idx as f32 * f32::consts::TAU / n_points as f32).sin(),
            )
        })
        .collect()
}

fn circular_track(
    n_points: u32,
    inner_radius: f32,
    outer_radius: f32,
) -> (Vec<Coords>, Vec<Coords>) {
    let inner_border: Vec<Coords> = circular_points(n_points, inner_radius);
    let outer_border: Vec<Coords> = circular_points(n_points, outer_radius);
    (inner_border, outer_border)
}

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;

pub fn main() {
    // Initialize SDL
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

    // Generate the circuit
    let n_points: u32 = 20;
    let inner_radius: f32 = 13.0;
    let outer_radius: f32 = 20.0;
    let (inner_border, outer_border) = circular_track(n_points, inner_radius, outer_radius);

    // Initialize a camera
    let camera: Camera = Camera {
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
        coords: Coords::init(0., 0.),
        px_per_meter: 14,
    };

    let mut event_pump: sdl3::EventPump = sdl_context.event_pump().unwrap();

    'running: loop {
        // Read the events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        // Fill the entire screen with RGB(50, 50, 50)
        canvas.set_draw_color(Color::RGB(50, 50, 50));
        canvas.clear();

        // Draw the circuit
        canvas.set_draw_color(Color::RGB(200, 200, 200));

        for (inn, out) in inner_border.iter().zip(outer_border.iter()) {
            inn.render(&camera, &mut canvas).unwrap();
            out.render(&camera, &mut canvas).unwrap();
        }

        for border in [&inner_border, &outer_border] {
            canvas
                .draw_lines(
                    border
                        .iter()
                        .chain(iter::once(border.first().unwrap()))
                        .map(|coord: &Coords| coord.as_fpoint(&camera))
                        .collect::<Vec<FPoint>>()
                        .as_slice(),
                )
                .unwrap();
        }

        // Apply the changes on the screen
        canvas.present();

        // Control the frame rate
        std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }
}
