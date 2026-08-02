use std::{
    f32::consts::{PI, TAU},
    ops::Div,
};

use bevy::prelude::*;
use rand::{Rng, RngExt, rng};
const BOX_WIDTH: f32 = 1600.0;
const BOX_HEIGHT: f32 = 900.0;
const BALL_RADIUS: f32 = 20.0;

#[derive(Resource)]
struct Physics {
    gravity: Vec2,
    density: f32,
    bounds: Vec2,
}

#[derive(Component)]
struct Ball;

#[derive(Component, Clone)]
struct Radius(f32);

#[derive(Component)]
struct Velocity(Vec2);

#[derive(Component)]
struct Stopped;

#[derive(Bundle)]
struct BallBundle {
    ball: Ball,
    radius: Radius,
    velocity: Velocity,
    mesh: Mesh2d,
    material: MeshMaterial2d<ColorMaterial>,
    transform: Transform,
}

impl BallBundle {
    fn new(
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
        pos: Vec2,
        vel: Vec2,
    ) -> Self {
        let mut rng = rand::rng();

        let radius = rng.random_range(12.0..28.0);

        // Pleasant pastel colors
        let color = Color::hsl(
            rng.random_range(0.0..360.0), // Any hue
            1.0,                          // High saturation
            0.7,                          // Fairly bright
        );
        Self {
            ball: Ball,
            radius: Radius(radius),
            velocity: Velocity(vel),
            mesh: Mesh2d(meshes.add(Circle::new(radius))),
            material: MeshMaterial2d(materials.add(color)),
            transform: Transform::from_translation(pos.extend(0.0)),
        }
    }
}

trait SpawnBall {
    fn spawn_ball(
        &mut self,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
        pos: Vec2,
    ) -> Entity;
}

impl SpawnBall for Commands<'_, '_> {
    fn spawn_ball(
        &mut self,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<ColorMaterial>,
        pos: Vec2,
    ) -> Entity {
        self.spawn((
            BallBundle::new(meshes, materials, pos, Vec2::new(50.0, 240.0)),
            Stopped,
        ))
        .id()
    }
}

fn main() {
    App::new()
        .insert_resource(Physics {
            gravity: Vec2::new(0.0, -980.0),
            density: 1.0,
            bounds: Vec2::new(BOX_WIDTH / 2.0, BOX_HEIGHT / 2.0),
        })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Bouncing Ball".into(),
                resolution: (BOX_WIDTH as u32, BOX_HEIGHT as u32).into(),
                ..default()
            }),
            ..default()
        }))
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                spawn_ball_on_click,
                move_ball_on_release,
                increase_size_on_hold,
                (move_balls, collision_detection).chain(),
            ),
        )
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn(Camera2d);

    commands.spawn_ball(&mut meshes, &mut materials, Vec2::ZERO);
}

fn spawn_ball_on_click(
    window: Single<&Window>,
    camera: Single<(&Camera, &GlobalTransform)>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }

    let Some(cursor) = window.cursor_position() else {
        return;
    };

    let (camera, transform) = *camera;

    let Ok(pos) = camera.viewport_to_world_2d(transform, cursor) else {
        return;
    };

    commands.spawn_ball(&mut meshes, &mut materials, pos);
}

fn move_ball_on_release(
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    balls: Query<Entity, With<Stopped>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }

    for ball in balls {
        commands.entity(ball).remove::<Stopped>();
    }
}

fn increase_size_on_hold(
    mouse: Res<ButtonInput<MouseButton>>,
    balls: Query<(Entity, &mut Transform, &mut Radius), With<Stopped>>,
) {
    if !mouse.pressed(MouseButton::Left) {
        return;
    }

    for mut ball in balls {
        ball.1.scale *= 1.01;
        ball.2.0 *= 1.01;
    }
}

fn move_balls(
    physics: Res<Physics>,
    time: Res<Time>,
    mut balls: Query<(&mut Transform, &mut Velocity, &Radius), (With<Ball>, Without<Stopped>)>,
) {
    let dt = time.delta_secs();

    for (mut transform, mut velocity, radius) in &mut balls {
        let _mass = radius.0 * physics.density;

        velocity.0 += physics.gravity * dt;
        transform.translation += (velocity.0 * dt).extend(0.0);

        let r = radius.0;
        let hw = physics.bounds.x;
        let hh = physics.bounds.y;

        if transform.translation.x + r > hw {
            transform.translation.x = hw - r;
            velocity.0.x *= -1.0;
        }

        if transform.translation.x - r < -hw {
            transform.translation.x = -hw + r;
            velocity.0.x *= -1.0;
        }

        if transform.translation.y + r > hh {
            transform.translation.y = hh - r;
            velocity.0.y *= -1.0;
        }

        if transform.translation.y - r < -hh {
            transform.translation.y = -hh + r;
            velocity.0.y *= -1.00;
        }
    }
}

fn collision_detection(
    mut balls: Query<(Entity, &mut Transform, &Radius, &mut Velocity), With<Ball>>,
) {
    // Snapshot the immutable data needed for broad-phase collision detection.
    //
    // We cannot hold mutable references to every ball while performing the
    // O(n²) pair search, so we first copy the positions and radii. Once a
    // collision is found, we fetch mutable references only for that pair.
    let snapshot: Vec<(Entity, Vec2, Radius)> = balls
        .iter()
        .map(|(entity, transform, radius, _)| {
            (entity, transform.translation.truncate(), radius.clone())
        })
        .collect();

    // Test every unique pair of balls.
    for i in 0..snapshot.len() {
        for j in i + 1..snapshot.len() {
            let (a_entity, a_pos, a_radius) = &snapshot[i];
            let (b_entity, b_pos, b_radius) = &snapshot[j];

            // Vector pointing from A -> B.
            let delta = b_pos - a_pos;

            // Two circles overlap when the distance between their centers
            // is less than the sum of their radii.
            let min_distance = a_radius.0 + b_radius.0;

            // Use squared distances to avoid an unnecessary square root.
            if delta.length_squared() > min_distance * min_distance {
                continue;
            }

            if let Ok(
                [
                    (_, mut a_transform, _, mut a_velocity),
                    (_, mut b_transform, _, mut b_velocity),
                ],
            ) = balls.get_many_mut([*a_entity, *b_entity])
            {
                let distance = delta.length();

                // Unit vector pointing from A -> B.
                //
                // Every collision impulse acts only along this direction.
                // The perpendicular (tangent) direction is unaffected because
                // we assume a perfectly frictionless collision.
                let normal = if distance > f32::EPSILON {
                    delta.normalize()
                } else {
                    // If both centers coincide there is no unique normal.
                    // Pick an arbitrary one to separate them.
                    Vec2::X
                };

                // Relative velocity of B with respect to A.
                let rel_vel = b_velocity.0 - a_velocity.0;

                // Project the relative velocity onto the collision normal.
                //
                // dot < 0 : balls are approaching.
                // dot > 0 : balls are already separating.
                //
                // If they're already separating we only need the positional
                // correction below; applying another impulse would inject
                // energy into the simulation.
                if rel_vel.dot(normal) > 0.0 {
                    continue;
                }

                let overlap = min_distance - distance;

                // Positional correction.
                //
                // Floating point error and discrete timesteps allow balls to
                // penetrate each other slightly. Move them apart before
                // computing the collision response.
                //
                // NOTE:
                // This equally splits the correction. A more realistic solver
                // would weight this by inverse mass.
                let correction = normal * (overlap * 0.5);

                a_transform.translation -= correction.extend(0.0);
                b_transform.translation += correction.extend(0.0);

                // Compute post-collision velocities using conservation of
                // momentum and kinetic energy.
                (a_velocity.0, b_velocity.0) = calc_final_vel(
                    (&a_radius, &a_velocity.0),
                    (&b_radius, &b_velocity.0),
                    &normal,
                );
            }
        }
    }
}

/// Computes the velocities after a perfectly elastic, frictionless collision.
///
/// The trick is to reduce the 2D problem into a 1D problem:
///
/// 1. Split each velocity into:
///    - a component along the collision normal
///    - a component perpendicular to the normal (tangent)
///
/// 2. Solve the 1D elastic collision only for the normal components using
///    conservation of momentum and kinetic energy.
///
/// 3. The tangential components are unchanged because there is no friction.
///
/// 4. Recombine the new normal component with the unchanged tangent component.
fn calc_final_vel(b1: (&Radius, &Vec2), b2: (&Radius, &Vec2), normal: &Vec2) -> (Vec2, Vec2) {
    // Projection of each velocity onto the collision normal.
    //
    // This is the only part affected by the collision.
    let u1_normal = normal * b1.1.dot(*normal);
    let u2_normal = normal * b2.1.dot(*normal);

    // Tangential components.
    //
    // Since the collision is frictionless these remain unchanged.
    let u1_tan = b1.1 - u1_normal;
    let u2_tan = b2.1 - u2_normal;

    let m1 = calc_mass(b1.0);
    let m2 = calc_mass(b2.0);

    // Standard 1D perfectly elastic collision equations.
    //
    // Derived from:
    //   - Conservation of momentum
    //   - Conservation of kinetic energy
    //
    // Only the normal components participate.
    let v1_n = ((m1 - m2) * u1_normal + 2.0 * m2 * u2_normal) / (m1 + m2);
    let v2_n = ((m2 - m1) * u2_normal + 2.0 * m1 * u1_normal) / (m1 + m2);

    // Reconstruct the final velocities.
    //
    // Final velocity =
    //      new normal component
    //    + unchanged tangential component.
    (v1_n + u1_tan, v2_n + u2_tan)
}

fn calc_mass(radius: &Radius) -> f32 {
    // Assume constant density in a 2D world.
    //
    // Mass is proportional to the circle's area:
    //      m = ρπr²
    //
    // Since density is constant, using r² alone would produce identical
    // collision results because the common factor cancels out.
    PI * radius.0 * radius.0
}
