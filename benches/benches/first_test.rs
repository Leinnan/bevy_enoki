use std::time::Duration;

use bevy::asset::Assets;
use bevy::color::LinearRgba;
use bevy::ecs::system::ResMut;
use bevy::ecs::world::World;
use bevy::prelude::*;
use bevy::tasks::{ComputeTaskPool, TaskPool};
use bevy_enoki::prelude::{ColorParticle2dMaterial, Rval};
use bevy_enoki::update::{self, ParticleEffectInstance, ParticleStore};
use bevy_enoki::{Particle2dEffect, ParticleEffectHandle, ParticleSpawner};
use criterion::*;

criterion_group!(benches, first_test);
criterion_main!(benches);

fn first_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("iter_fragmented(4096)_empty");
    group.warm_up_time(core::time::Duration::from_millis(500));
    for input in [
        (0.10, 1000),
        (0.10, 2000),
        (0.10, 3000),
        (0.20, 4000),
        (0.30, 5000),
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("duration{:.2}amount{}", input.0, input.1)),
            &input,
            |b, i| {
                b.iter_batched_ref(
                    || {
                        ComputeTaskPool::get_or_init(TaskPool::default);
                        let mut time = Time::<Virtual>::default();
                        time.advance_by(Duration::from_secs_f32(0.3));
                        let mut world = World::new();
                        world.init_resource::<Assets<ColorParticle2dMaterial>>();
                        world.init_resource::<Assets<Particle2dEffect>>();
                        {
                            let registry = world.get_resource_or_init::<AppTypeRegistry>();

                            // Recursively register all components in the bundle to the reflection type registry.
                            {
                                let mut r = registry.write();
                                r.register::<update::ParticleStore>();
                                r.register::<update::ParticleSpawnerState>();
                                r.register::<update::Particle>();
                                r.register::<ParticleEffectHandle>();
                                r.register::<ParticleSpawner<ColorParticle2dMaterial>>();
                            }
                        }
                        world.insert_resource(time);
                        _ = world.run_system_cached_with(load_assets, *i);
                        world.flush();
                        std::hint::black_box(world)
                    },
                    |world| {
                        for _ in 0..50 {
                            _ = world.run_system_cached(bevy_enoki::update::update_spawner);
                        }
                        world.flush();
                        let amount: usize = world
                            .query::<&ParticleStore>()
                            .iter(world)
                            .map(|c| c.len())
                            .sum();
                        amount
                    },
                    criterion::BatchSize::NumIterations(100),
                )
            },
        );
    }
    group.finish();
}

fn load_assets(
    In(input): In<(f32, u32)>,
    mut materials: ResMut<Assets<ColorParticle2dMaterial>>,
    mut particles: ResMut<Assets<Particle2dEffect>>,
    mut cmd: Commands,
) {
    let particle = Particle2dEffect {
        spawn_rate: input.0,
        spawn_amount: input.1,
        emission_shape: bevy_enoki::EmissionShape::Point,
        lifetime: Rval::new(5.0, 0.0),
        ..Default::default()
    };
    let handle = ParticleEffectHandle(particles.add(particle.clone()));
    let particle_spawner =
        ParticleSpawner(materials.add(ColorParticle2dMaterial::new(LinearRgba::WHITE)));
    cmd.spawn((
        ParticleEffectInstance(Some(particle)),
        GlobalTransform::default(),
        Transform::default(),
        handle,
        particle_spawner,
        ParticleStore::default(),
    ));
}
