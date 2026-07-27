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

#[derive(Component)]
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
        let color = Color::srgb(
            rng.random_range(0.3..1.0),
            rng.random_range(0.3..1.0),
            rng.random_range(0.3..1.0),
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
            BallBundle::new(meshes, materials, pos, Vec2::new(125.0, 240.0)),
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
                collision_detection,
                move_balls,
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

    println!("pressed");

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
    let snapshot: Vec<_> = balls
        .iter()
        .map(|(entity, transform, radius, _)| (entity, transform.translation.truncate(), radius.0))
        .collect();

    for i in 0..snapshot.len() {
        for j in i + 1..snapshot.len() {
            let (a_entity, a_pos, a_radius) = snapshot[i];
            let (b_entity, b_pos, b_radius) = snapshot[j];

            let delta = b_pos - a_pos;
            let min_distance = a_radius + b_radius;

            if delta.length_squared() > min_distance * min_distance {
                continue;
            }

            if let Ok(
                [
                    (_, mut a_transform, _, mut a_velocity),
                    (_, mut b_transform, _, mut b_velocity),
                ],
            ) = balls.get_many_mut([a_entity, b_entity])
            {
                let distance = delta.length();

                // Avoid division by zero when balls are exactly on top of each other.
                let normal = if distance > f32::EPSILON {
                    delta.normalize()
                } else {
                    Vec2::X
                };

                let overlap = min_distance - distance;

                // Push both balls apart equally.
                let correction = normal * (overlap * 0.5);

                a_transform.translation -= correction.extend(0.0);
                b_transform.translation += correction.extend(0.0);

                // Keep your current collision response.
                a_velocity.0 *= -1.0;
                b_velocity.0 *= -1.0;
            }
        }
    }
}
