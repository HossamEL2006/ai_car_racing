use core::f32;
use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::pixels::Color;
use sdl3::rect::{Point, Rect};
use sdl3::render::{Canvas, FPoint};
use sdl3::video::Window;
use std::time::Duration;
use std::{iter, ops};

#[derive(Clone, Copy)]
struct Meter(f32);

impl ops::Add for Meter {
    type Output = Meter;

    fn add(self, rhs: Self) -> Self::Output {
        Meter(self.0 + rhs.0)
    }
}

impl ops::Sub for Meter {
    type Output = Meter;

    fn sub(self, rhs: Self) -> Self::Output {
        Meter(self.0 - rhs.0)
    }
}

impl ops::Neg for Meter {
    type Output = Meter;

    fn neg(self) -> Self::Output {
        Meter(-self.0)
    }
}

#[derive(Clone, Copy)]
struct Coords {
    x: Meter,
    y: Meter,
}

#[derive(Clone, Copy)]
struct Vector {
    x: Meter,
    y: Meter,
}

impl ops::Sub for Coords {
    type Output = Vector;

    fn sub(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl ops::Add<Vector> for Coords {
    type Output = Coords;

    fn add(self, rhs: Vector) -> Self::Output {
        Coords {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
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
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
        FPoint::new(
            (camera.width / 2) as f32 + px_coords.x.0,
            (camera.height / 2) as f32 - px_coords.y.0,
        )
    }

    fn as_point(&self, camera: &Camera) -> Point {
        let px_coords: Vector = (camera.px_per_meter as f32) * (*self - camera.coords);
        Point::new(
            (camera.width / 2) as i32 + (px_coords.x.0).round() as i32,
            (camera.height / 2) as i32 - (px_coords.y.0).round() as i32,
        )
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        canvas.draw_rect(Rect::from_center(self.as_point(camera), 5, 5))
    }
}

impl Vector {
    fn init(x: f32, y: f32) -> Vector {
        Vector {
            x: Meter(x),
            y: Meter(y),
        }
    }

    fn orthogonal(&self) -> Vector {
        Vector {
            x: -self.y,
            y: self.x,
        }
    }

    fn norm(&self) -> Meter {
        Meter((self.x.0.powi(2) + self.y.0.powi(2)).sqrt())
    }

    fn normalized(&self) -> Vector {
        self.norm().0.recip() * *self
    }
}

impl ops::Add for Vector {
    type Output = Vector;

    fn add(self, rhs: Self) -> Self::Output {
        Vector {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl ops::Mul<Vector> for f32 {
    type Output = Vector;

    fn mul(self, mut rhs: Vector) -> Self::Output {
        rhs.x.0 *= self;
        rhs.y.0 *= self;
        rhs
    }
}

struct Camera {
    width: u32,
    height: u32,
    coords: Coords,
    px_per_meter: u32,
}

struct Car {
    coords: Coords,
    direction_vec: Vector,
    length: Meter,
    width: Meter,
    acceleration: Vector,
    velocity: Vector,
}

impl Car {
    fn init(x: f32, y: f32, direction: Vector) -> Car {
        Car {
            coords: Coords::init(x, y),
            direction_vec: direction.normalized(),
            length: Meter(4.5),
            width: Meter(2.),
            acceleration: Vector::init(0., 0.),
            velocity: Vector::init(0., 0.),
        }
    }

    fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        let corners: Vec<FPoint> = [(1., 1.), (-1., 1.), (-1., -1.), (1., -1.), (1., 1.)]
            .into_iter()
            .map(|(d1, d2)| {
                self.coords
                    + (d1 * (self.length.0 / 2.)) * self.direction_vec
                    + (d2 * (self.width.0 / 2.)) * self.direction_vec.orthogonal()
            })
            .map(|coords: Coords| coords.as_fpoint(camera))
            .collect();

        canvas.draw_lines(corners.as_slice())
    }

    fn set_acceleration(&mut self, acc: Vector) {
        self.acceleration = acc
    }

    fn update_coords(&mut self, dt: f32) {
        self.velocity = self.velocity + dt * self.acceleration;
        let velocity_norm: f32 = self.velocity.norm().0;
        if velocity_norm != 0. {
            self.direction_vec = (1. / velocity_norm) * self.velocity;
        }
        self.coords = self.coords + dt * self.velocity;
    }
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

const GAME_FPS: f32 = 60.0;
const DT: f32 = GAME_FPS.recip();

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

    // Initialize a car
    let mut car: Car = Car::init(0., -16.5, Vector::init(1., 0.));
    car.set_acceleration(Vector::init(50.0, 0.0));

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

        // Update the car's position
        if car.velocity.norm().0 > 10. {
            car.set_acceleration(0.7 * car.velocity.orthogonal());
        }
        car.update_coords(DT);

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

        // Render the car
        canvas.set_draw_color(Color::RGB(200, 0, 0));
        car.render(&camera, &mut canvas).unwrap();

        // Apply the changes on the screen
        canvas.present();

        // Control the frame rate
        std::thread::sleep(Duration::new(
            0,
            (1_000_000_000f32 / GAME_FPS).round() as u32,
        ));
    }
}
