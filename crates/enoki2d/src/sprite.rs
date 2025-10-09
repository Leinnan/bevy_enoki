use bevy_asset::{Asset, Handle};
use bevy_image::{Image, TextureAtlasLayout};
use bevy_math::{UVec4, Vec4};
use bevy_reflect::TypePath;
use bevy_render::render_resource::AsBindGroup;

use crate::PARTICLE_ATLAS_SPRITE_FRAG;

use super::{Particle2dMaterial, PARTICLE_SPRITE_FRAG};

/// Sprite Material lets you add textures and animations
/// to particles.
#[derive(AsBindGroup, Asset, TypePath, Clone)]
pub struct SpriteParticle2dMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Option<Handle<Image>>,
    #[uniform(2)]
    frame_data: UVec4,
}
/// Atlas Material lets you specify a part of the texture
/// to particles.
#[derive(AsBindGroup, Asset, TypePath, Clone)]
pub struct AtlasParticle2dMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub texture: Option<Handle<Image>>,
    #[uniform(2)]
    frame_data: Vec4,
}

impl Default for SpriteParticle2dMaterial {
    fn default() -> Self {
        Self {
            texture: None,
            frame_data: UVec4::ONE,
        }
    }
}

impl SpriteParticle2dMaterial {
    pub fn new(texture: Handle<Image>, max_hframes: u32, max_vframes: u32) -> Self {
        Self {
            texture: Some(texture),
            frame_data: UVec4::new(max_hframes, max_vframes, 0, 0),
        }
    }

    pub fn from_texture(texture: Handle<Image>) -> Self {
        Self {
            texture: Some(texture),
            frame_data: UVec4::new(1, 1, 0, 0),
        }
    }
}

impl AtlasParticle2dMaterial {
    pub fn from_layout(texture: Handle<Image>, layout: TextureAtlasLayout, index: usize) -> Self {
        let frame_data = if let Some(s) = layout.textures.get(index) {
            let start_x = s.min.x as f32 / layout.size.x as f32;
            let end_x = s.max.x as f32 / layout.size.x as f32;
            let start_y = s.min.y as f32 / layout.size.y as f32;
            let end_y = s.max.y as f32 / layout.size.y as f32;
            Vec4::new(start_x, start_y, end_x, end_y)
        } else {
            Vec4::new(0.0, 0.0, 1.0, 1.0)
        };
        Self {
            texture: Some(texture),
            frame_data,
        }
    }
    pub fn from_vec(texture: Handle<Image>, frame_data: Vec4) -> Self {
        Self {
            texture: Some(texture),
            frame_data,
        }
    }
}

impl Particle2dMaterial for SpriteParticle2dMaterial {
    fn fragment_shader() -> bevy_shader::ShaderRef {
        PARTICLE_SPRITE_FRAG.into()
    }
}
impl Particle2dMaterial for AtlasParticle2dMaterial {
    fn fragment_shader() -> bevy_shader::ShaderRef {
        PARTICLE_ATLAS_SPRITE_FRAG.into()
    }
}
