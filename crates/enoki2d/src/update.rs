use super::{prelude::EmissionShape, Particle2dEffect, ParticleEffectHandle};
use crate::{material::InstanceData, values::Random};
use bevy::{
    prelude::*,
    render::primitives::Aabb,
    tasks::{ComputeTaskPool, ParallelSliceMut},
};
use std::{ops::AddAssign, time::Duration};

/// Tag Component, deactivates spawner after the first
/// spawning of particles
#[derive(Component, Default)]
pub enum OneShot {
    #[default]
    Deactivate,
    Despawn,
}

/// Spawner states controls the spawner
#[derive(Component, Clone, Debug, Reflect)]
pub struct ParticleSpawnerState {
    pub max_particles: u32,
    pub active: bool,
    pub timer: Timer,
}

/// A clone of the asset, unique to each spawner
/// that can be mutated by any system.
/// Any change to `Particle2dEffect` Asset will
/// re-copy the effect.
#[derive(Component, Deref, DerefMut, Default)]
pub struct ParticleEffectInstance(pub Option<Particle2dEffect>);

impl Default for ParticleSpawnerState {
    fn default() -> Self {
        Self {
            active: true,
            max_particles: u32::MAX,
            timer: Timer::new(Duration::ZERO, TimerMode::Repeating),
        }
    }
}

/// Component for storing particle data
#[derive(Component, Default, Reflect, Deref, DerefMut)]
pub struct ParticleStore {
    #[deref]
    pub particles: Vec<(Particle, InstanceData)>,
}

#[derive(Clone, Reflect)]
pub struct Particle {
    pub(crate) start_pos: Vec3,
    pub(crate) direction: Vec2,
    pub(crate) scale: f32,
    pub(crate) gravity: Vec3,
    pub(crate) duration: f32,
    pub(crate) duration_fraction: f32,
    pub(crate) velocity: Vec4,
    pub(crate) color: LinearRgba,
    pub(crate) frame: u32,
    pub(crate) linear_acceleration: f32,
    pub(crate) linear_damp: f32,
    pub(crate) angular_acceleration: f32,
    pub(crate) angular_damp: f32,
}

impl Particle {
    pub fn get_transform(&self) -> Transform {
        let mut transform = Transform::from_translation(self.start_pos);
        transform.scale = Vec3::splat(self.scale);

        let progress = self.duration_fraction;

        // Cache commonly used values
        let lin_velo = self.velocity.xyz();
        let angular_vel = self.velocity.w;

        // Apply damping (exponential decay)
        let lin_damp_factor = (-self.linear_damp * progress * self.duration).exp();
        let ang_damp_factor = (-self.angular_damp * progress * self.duration).exp();

        // Apply acceleration over time
        let lin_accel_contribution = self.linear_acceleration * progress * self.duration;
        let ang_accel_contribution = self.angular_acceleration * progress * self.duration;

        let new_lin_velo =
            lin_velo * lin_damp_factor + self.direction.extend(0.0) * lin_accel_contribution;
        let new_angular_vel = angular_vel * ang_damp_factor + ang_accel_contribution;

        // Calculate displacement using physics integration
        let time_step = progress * self.duration;
        let displacement = new_lin_velo * time_step + 0.5 * self.gravity * time_step * time_step;

        // Update position with displacement
        transform.translation += displacement;
        transform.rotate_local_z(new_angular_vel * time_step);
        transform
    }
}

pub(crate) fn clone_effect(
    mut particle_spawners: Query<
        (&mut ParticleEffectInstance, &ParticleEffectHandle),
        Added<ParticleSpawnerState>,
    >,
    effects: Res<Assets<Particle2dEffect>>,
) {
    particle_spawners
        .iter_mut()
        .for_each(|(mut effect_overwrites, handle)| {
            let Some(effect) = effects.get(&handle.0) else {
                return;
            };

            effect_overwrites.0 = Some(effect.clone());
        });
}

pub(crate) fn remove_finished_spawner(
    mut cmd: Commands,
    spawner: Query<(Entity, &ParticleStore, &ParticleSpawnerState, &OneShot)>,
) {
    spawner
        .iter()
        .for_each(|(entity, store, controller, one_shot)| {
            if matches!(one_shot, OneShot::Despawn) && !controller.active && store.is_empty() {
                cmd.entity(entity).despawn();
            }
        })
}

pub fn update_spawner(
    mut particles: Query<(
        Entity,
        &mut ParticleStore,
        &mut ParticleSpawnerState,
        &ParticleEffectInstance,
        &GlobalTransform,
    )>,
    one_shots: Query<&OneShot>,
    time: Res<Time<Virtual>>,
) {
    particles.par_iter_mut().for_each(
        |(entity, mut store, mut state, effect_instance, transform)| {
            if state.max_particles <= store.particles.len() as u32 {
                return;
            }

            let Some(effect) = &effect_instance.0 else {
                return;
            };

            let transform = transform.compute_transform();

            state
                .timer
                .set_duration(Duration::from_secs_f32(effect.spawn_rate));
            state.timer.tick(time.delta());

            if state.timer.finished() && state.active {
                for _ in 0..effect.spawn_amount {
                    let particle = create_particle(effect, &transform);
                    let instance_data: InstanceData = (&particle).into();
                    store.push((particle, instance_data));
                }

                if one_shots.get(entity).is_ok() {
                    state.active = false;
                }
            }
            let delta = time.delta_secs();
            match (effect.scale_curve.as_ref(), effect.color_curve.as_ref()) {
                (None, None) => {
                    store.par_splat_map_mut(ComputeTaskPool::get(), None, |_, particles| {
                        for (particle, instance_data) in particles.iter_mut() {
                            particle
                                .duration_fraction
                                .add_assign(delta / particle.duration);
                            instance_data.update_duration_fraction(particle.duration_fraction);
                            instance_data.update_transform(&particle);
                        }
                    });
                }
                (None, Some(color_curve)) => {
                    store.par_splat_map_mut(ComputeTaskPool::get(), None, |_, particles| {
                        for (particle, instance_data) in particles.iter_mut() {
                            particle
                                .duration_fraction
                                .add_assign(delta / particle.duration);
                            instance_data.update_transform(&particle);
                            particle.color = color_curve.lerp(particle.duration_fraction);
                            instance_data.update_duration_fraction(particle.duration_fraction);
                            instance_data.update_color(&particle.color);
                        }
                    });
                }
                (Some(scale_curve), None) => {
                    store.par_splat_map_mut(ComputeTaskPool::get(), None, |_, particles| {
                        for (particle, instance_data) in particles.iter_mut() {
                            particle
                                .duration_fraction
                                .add_assign(delta / particle.duration);
                            particle.scale = scale_curve.lerp(particle.duration_fraction);
                            instance_data.update_duration_fraction(particle.duration_fraction);
                            instance_data.update_transform(&particle);
                        }
                    });
                }
                (Some(scale_curve), Some(color_curve)) => {
                    store.par_splat_map_mut(ComputeTaskPool::get(), None, |_, particles| {
                        for (particle, instance_data) in particles.iter_mut() {
                            particle
                                .duration_fraction
                                .add_assign(delta / particle.duration);
                            particle.scale = scale_curve.lerp(particle.duration_fraction);
                            particle.color = color_curve.lerp(particle.duration_fraction);
                            instance_data.update_duration_fraction(particle.duration_fraction);
                            instance_data.update_transform(&particle);
                            instance_data.update_color(&particle.color);
                        }
                    });
                }
            };
            store.retain(|(particle, _)| particle.duration_fraction < 1.0);
        },
    );
}

fn create_particle(effect: &Particle2dEffect, transform: &Transform) -> Particle {
    // direction
    let direction = effect
        .direction
        .as_ref()
        .map(|m| m.rand())
        .unwrap_or_default()
        .normalize_or_zero();

    // apply local rotation
    let direction = direction.rotate(transform.right().truncate());

    // speed
    let speed = effect
        .linear_speed
        .as_ref()
        .map(|s| s.rand())
        .unwrap_or_default();
    // angular
    let angular = effect
        .angular_speed
        .as_ref()
        .map(|s| s.rand())
        .unwrap_or_default();
    // angular
    let scale = effect.scale.as_ref().map(|s| s.rand()).unwrap_or_default();

    let gravity_direction = effect
        .gravity_direction
        .as_ref()
        .map(|g| g.rand())
        .unwrap_or_default()
        .normalize_or_zero()
        .extend(0.);

    let gravity_speed = effect
        .gravity_speed
        .as_ref()
        .map(|g| g.rand())
        .unwrap_or_default();

    let linear_damp = effect
        .linear_damp
        .as_ref()
        .map(|d| d.rand())
        .unwrap_or_default();

    let angular_damp = effect
        .angular_damp
        .as_ref()
        .map(|a| a.rand())
        .unwrap_or_default();

    let angular_acceleration = effect
        .angular_acceleration
        .as_ref()
        .map(|a| a.rand())
        .unwrap_or_default();

    let linear_acceleration = effect
        .linear_acceleration
        .as_ref()
        .map(|a| a.rand())
        .unwrap_or_default();

    let mut transform = transform.translation;

    transform += match effect.emission_shape {
        EmissionShape::Point => Vec3::ZERO,
        EmissionShape::Circle(radius) => {
            Vec3::new(rand::random::<f32>() - 0.5, rand::random::<f32>() - 0.5, 0.)
                .normalize_or_zero()
                * radius
                * rand::random::<f32>()
        }
    };

    Particle {
        start_pos: transform,
        scale,
        direction,
        velocity: (direction * speed).extend(0.).extend(angular),
        duration_fraction: 0.0,
        duration: effect.lifetime.rand(),
        color: effect.color.unwrap_or(LinearRgba::WHITE),
        angular_damp,
        linear_damp,
        angular_acceleration,
        linear_acceleration,
        gravity: gravity_direction * gravity_speed,
        frame: 0,
    }
}

pub(crate) fn calculcate_particle_bounds(
    mut cmd: Commands,
    spawners: Query<(Entity, &ParticleStore), Without<crate::NoAutoAabb>>,
) {
    spawners.iter().for_each(|(entity, store)| {
        if store.is_empty() {
            return;
        }
        let accuracy = (store.len() / 1000).clamp(1, 10);

        let (min, max) = store
            .iter()
            .enumerate()
            .filter(|(i, _)| i % accuracy == 0)
            .fold((Vec2::ZERO, Vec2::ZERO), |mut acc, (_, (particle, _))| {
                acc.0.x = acc.0.x.min(particle.start_pos.x);
                acc.0.y = acc.0.y.min(particle.start_pos.y);
                acc.1.x = acc.1.x.max(particle.start_pos.x);
                acc.1.y = acc.1.y.max(particle.start_pos.y);
                acc
            });
        cmd.entity(entity)
            .try_insert(Aabb::from_min_max(min.extend(0.), max.extend(0.)));
    });
}
