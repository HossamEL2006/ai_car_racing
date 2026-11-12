use crate::world_objects::WorldPolygon;
use sdl3::{pixels::Color, render::Texture};
use std::fmt::Debug;
use std::hash::Hash;
use std::{collections::HashMap, rc::Rc};

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub struct Hashf32(u32);

impl Hashf32 {
    pub fn new(value: f32) -> Self {
        Self(value.to_bits())
    }

    pub fn value(&self) -> f32 {
        f32::from_bits(self.0)
    }
}

impl Debug for Hashf32 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value())
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub(crate) enum HashableGraphicProps {
    Circle {
        radius: Hashf32,
        color: Color,
    },
    Disc {
        radius: Hashf32,
        color: Color,
    },
    WorldVector {
        x: Hashf32,
        y: Hashf32,
        px_per_meter: u32,
        color: Option<Color>,
        label: Option<String>,
    },
    WorldPolygon {
        polygon: WorldPolygon,
        px_per_meter: u32,
        color: Color,
        filled: bool,
    },
    Text {
        text: String,
        font_name: String,
        color: Color,
    },
}

// TODO: Consider including the texture creator in the struct...
//       ...if not doing so can cause an invalid texture render (when working with multiple canvas)
pub struct TextureCache<'tc, HashType> {
    texture_cache: HashMap<HashType, Rc<Texture<'tc>>>,
}

impl<'tc, HashType: Hash + Eq> TextureCache<'tc, HashType> {
    pub fn new() -> TextureCache<'tc, HashType> {
        Self {
            texture_cache: HashMap::new(),
        }
    }

    pub fn insert(&mut self, hash: HashType, texture: Texture<'tc>) {
        self.texture_cache.insert(hash, Rc::new(texture));
    }

    pub fn get_cached(&self, hash: &HashType) -> Option<Rc<Texture<'tc>>> {
        self.texture_cache.get(hash).cloned()
    }

    pub fn len(&self) -> usize {
        self.texture_cache.len()
    }
}

impl<'tc, HashType: Hash + Eq + Debug> Debug for TextureCache<'tc, HashType> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // debug a list of HashTypes
        f.debug_struct("TextureCache")
            .field("hashes", &self.texture_cache.keys().collect::<Vec<_>>())
            .finish()
    }
}
