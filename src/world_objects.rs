use crate::world_graphic::{WorldPolygonGraphic, WorldVectorGraphic};
use sdl3::{
    pixels::Color,
    rect,
    render::{self, Canvas},
    video::Window,
};
use std::hash::Hash;
use std::ops;

// --------------------
// Hash Float
// --------------------

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
struct Hashf32(u32);

impl Hashf32 {
    fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    fn value(&self) -> f32 {
        f32::from_bits(self.0)
    }
}

// --------------------
// Meter
// --------------------

#[derive(Clone, Copy, PartialOrd)]
pub struct Meter(f32);

impl Meter {
    pub fn new(value: f32) -> Meter {
        Meter(value)
    }

    pub fn value(&self) -> f32 {
        self.0
    }

    pub fn as_px_dist(&self, camera: &Camera) -> f32 {
        self.0 * camera.px_per_meter as f32
    }
}

impl PartialEq for Meter {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for Meter {}

impl Hash for Meter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

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

impl ops::Mul<Meter> for f32 {
    type Output = Meter;

    fn mul(self, rhs: Meter) -> Self::Output {
        Meter(self * rhs.0)
    }
}

// --------------------
// WorldCoords
// --------------------

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct WorldCoords {
    x: Meter,
    y: Meter,
}

impl WorldCoords {
    pub fn new(x: Meter, y: Meter) -> WorldCoords {
        WorldCoords { x, y }
    }

    pub fn origin() -> WorldCoords {
        WorldCoords {
            x: Meter::new(0.),
            y: Meter::new(0.),
        }
    }

    pub fn x(&self) -> Meter {
        self.x
    }

    pub fn y(&self) -> Meter {
        self.y
    }

    pub fn as_fpoint(&self, camera: &Camera) -> render::FPoint {
        let px_coords: WorldVector = WorldVector {
            x: Meter::new((camera.px_per_meter as f32) * (self.x - camera.coords.x).0),
            y: Meter::new((camera.px_per_meter as f32) * (self.y - camera.coords.y).0),
        };
        render::FPoint::new(
            (camera.width / 2) as f32 + px_coords.x.0,
            (camera.height / 2) as f32 - px_coords.y.0,
        )
    }

    pub fn as_point(&self, camera: &Camera) -> rect::Point {
        let px_coords: WorldVector = (camera.px_per_meter as f32) * (*self - camera.coords);
        rect::Point::new(
            (camera.width / 2) as i32 + (px_coords.x.0).round() as i32,
            (camera.height / 2) as i32 - (px_coords.y.0).round() as i32,
        )
    }

    pub fn render(&self, camera: &Camera, canvas: &mut Canvas<Window>) -> Result<(), sdl3::Error> {
        canvas.draw_rect(rect::Rect::from_center(self.as_point(camera), 5, 5))
    }
}

// --------------------
// WorldVector
// --------------------

#[derive(Clone, Copy, PartialEq)]
pub struct WorldVector {
    x: Meter,
    y: Meter,
}

impl WorldVector {
    pub fn new(x: Meter, y: Meter) -> WorldVector {
        WorldVector { x, y }
    }

    pub fn zero() -> WorldVector {
        WorldVector {
            x: Meter::new(0.),
            y: Meter::new(0.),
        }
    }

    pub fn x(&self) -> Meter {
        self.x
    }

    pub fn y(&self) -> Meter {
        self.y
    }

    pub fn angle(&self) -> f32 {
        self.y.0.atan2(self.x.0)
    }

    pub fn orthogonal(&self) -> WorldVector {
        WorldVector {
            x: Meter::new(-self.y.0),
            y: Meter::new(self.x.0),
        }
    }

    pub fn norm(&self) -> Meter {
        Meter::new((self.x.0.powi(2) + self.y.0.powi(2)).sqrt())
    }

    pub fn normalized(&self) -> WorldVector {
        self.norm().0.recip() * *self
    }

    pub fn rotated(&self, theta: f32) -> WorldVector {
        let cos_theta: f32 = theta.cos();
        let sin_theta: f32 = theta.sin();

        let x: f32 = self.x.0;
        let y: f32 = self.y.0;

        WorldVector {
            x: Meter::new(x * cos_theta - y * sin_theta),
            y: Meter::new(x * sin_theta + y * cos_theta),
        }
    }

    pub fn into_graphic(&self, base: WorldCoords, color: Color) -> WorldVectorGraphic {
        WorldVectorGraphic {
            base,
            vector: *self,
            color,
        }
    }
}

impl ops::Sub for WorldCoords {
    type Output = WorldVector;

    fn sub(self, rhs: Self) -> Self::Output {
        WorldVector {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl ops::Add for WorldVector {
    type Output = WorldVector;

    fn add(self, rhs: Self) -> Self::Output {
        WorldVector {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

// WorldCoords + WorldVector Operation
impl ops::Add<WorldVector> for WorldCoords {
    type Output = WorldCoords;

    fn add(self, rhs: WorldVector) -> Self::Output {
        WorldCoords {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

// f32 * WorldVector Operation
impl ops::Mul<WorldVector> for f32 {
    type Output = WorldVector;

    fn mul(self, mut rhs: WorldVector) -> Self::Output {
        rhs.x.0 *= self;
        rhs.y.0 *= self;
        rhs
    }
}

// --------------------
// WorldPolygon
// --------------------

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct WorldPolygon {
    vertices: Vec<WorldCoords>,
}

impl WorldPolygon {
    pub fn new(vertices: &[WorldCoords]) -> Self {
        Self {
            vertices: vertices.to_vec(),
        }
    }

    pub fn vertices(&self) -> &[WorldCoords] {
        &self.vertices
    }

    pub fn vertices_mut(&mut self) -> &mut [WorldCoords] {
        &mut self.vertices
    }

    pub fn into_vertices(self) -> Vec<WorldCoords> {
        self.vertices
    }

    pub fn min_x(&self) -> Meter {
        Meter::new(self.vertices.iter().map(|v| v.x.0).fold(f32::MAX, f32::min))
    }

    pub fn max_x(&self) -> Meter {
        Meter::new(self.vertices.iter().map(|v| v.x.0).fold(f32::MIN, f32::max))
    }

    pub fn min_y(&self) -> Meter {
        Meter::new(self.vertices.iter().map(|v| v.y.0).fold(f32::MAX, f32::min))
    }

    pub fn max_y(&self) -> Meter {
        Meter::new(self.vertices.iter().map(|v| v.y.0).fold(f32::MIN, f32::max))
    }

    pub fn width(&self) -> Meter {
        self.max_x() - self.min_x()
    }

    pub fn height(&self) -> Meter {
        self.max_y() - self.min_y()
    }

    pub fn contains_point(&self, point: WorldCoords) -> bool {
        let mut inside: bool = false;
        let n: usize = self.vertices.len();
        let mut j: usize = n - 1;
        for i in 0..n {
            let pi: WorldCoords = self.vertices[i];
            let pj: WorldCoords = self.vertices[j];
            if ((pi.y.0 > point.y.0) != (pj.y.0 > point.y.0))
                && (point.x.0
                    < (pj.x.0 - pi.x.0) * (point.y.0 - pi.y.0) / (pj.y.0 - pi.y.0) + pi.x.0)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    }

    pub fn as_graphic(&self, filled: bool, color: Color) -> WorldPolygonGraphic {
        WorldPolygonGraphic {
            polygon: self.clone(),
            filled,
            color,
        }
    }
}

// --------------------
// Camera
// --------------------

pub struct Camera {
    width: u32,
    height: u32,
    coords: WorldCoords,
    px_per_meter: u32, // TODO: Consider making this a float
}

impl Camera {
    pub fn new(width: u32, height: u32, coords: WorldCoords, px_per_meter: u32) -> Self {
        Self {
            width,
            height,
            coords,
            px_per_meter,
        }
    }

    pub fn px_per_meter(&self) -> u32 {
        self.px_per_meter
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn coords(&self) -> WorldCoords {
        self.coords
    }

    pub fn set_coords(&mut self, coords: WorldCoords) {
        self.coords = coords;
    }
}
