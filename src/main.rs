extern crate sdl2;

use std::time::{Duration, Instant};

use rand::distributions::Uniform;
use rand::Rng;

use sdl2::event::{Event, WindowEvent};
use sdl2::gfx::primitives::DrawRenderer;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::{Point, Rect};
use sdl2::render::{Canvas, Texture, TextureCreator};

use sdl2::video::{Window, WindowContext};
use sdl2::EventPump;

const WHITE: Color = Color {
    r: 0xff,
    g: 0xff,
    b: 0xff,
    a: 0xff,
};
const BLACK: Color = Color {
    r: 0x00,
    g: 0x00,
    b: 0x00,
    a: 0xff,
};

const NANOS_PER_SEC: u32 = 1_000_000_000;
fn f64_duration(duration: &Duration) -> f64 {
    (duration.as_secs() as f64) + (duration.subsec_nanos() as f64) / (NANOS_PER_SEC as f64)
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum KeyStatus {
    Up,
    Held,
    Down,
}
impl KeyStatus {
    fn step(&mut self) {
        match *self {
            KeyStatus::Down => *self = KeyStatus::Held,
            KeyStatus::Held => *self = KeyStatus::Held,
            KeyStatus::Up => *self = KeyStatus::Up,
        }
    }
    fn down(&self) -> bool {
        match *self {
            KeyStatus::Down => true,
            KeyStatus::Held => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
struct Keys {
    thrust: KeyStatus,
    left: KeyStatus,
    right: KeyStatus,
    fire: KeyStatus,
    pause: KeyStatus,
    quit: KeyStatus,
}
impl Keys {
    fn new() -> Self {
        Keys {
            thrust: KeyStatus::Up,
            left: KeyStatus::Up,
            right: KeyStatus::Up,
            fire: KeyStatus::Up,
            pause: KeyStatus::Up,
            quit: KeyStatus::Up,
        }
    }
    fn with_events(&mut self, event_pump: &mut EventPump) {
        event_pump.poll_iter().for_each(|event| {
            let value;
            match event {
                Event::KeyDown { repeat: false, .. } => value = KeyStatus::Down,
                Event::KeyUp { repeat: false, .. } => value = KeyStatus::Up,
                _ => return,
            }

            match event {
                Event::KeyDown {
                    keycode: Some(key),
                    repeat: false,
                    ..
                }
                | Event::KeyUp {
                    keycode: Some(key),
                    repeat: false,
                    ..
                } => match key {
                    Keycode::Up => self.thrust = value,
                    Keycode::Left => self.left = value,
                    Keycode::Right => self.right = value,
                    Keycode::Space => self.fire = value,
                    Keycode::P => self.pause = value,
                    Keycode::Q => self.quit = value,
                    Keycode::Escape => self.quit = value,
                    _ => (),
                },
                Event::Quit { .. } => self.quit = KeyStatus::Down,
                Event::Window {
                    win_event: WindowEvent::Close,
                    ..
                } => self.quit = KeyStatus::Down,
                _ => (),
            }
        })
    }
}

#[derive(Debug, Clone)]
struct V2(pub f64, pub f64);
impl V2 {
    const ZERO: V2 = V2(0.0, 0.0);
}
impl Into<Point> for V2 {
    fn into(self) -> Point {
        Point::new(self.0 as i32, self.1 as i32)
    }
}
impl std::ops::Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, rhs: f64) -> V2 {
        V2(self.0 * rhs, self.1 * rhs)
    }
}
impl std::ops::Rem for V2 {
    type Output = V2;
    fn rem(self, rhs: V2) -> V2 {
        V2((self.0 + rhs.0) % rhs.0, (self.1 + rhs.1) % rhs.1)
    }
}
impl std::ops::Add for V2 {
    type Output = V2;
    fn add(self, rhs: V2) -> V2 {
        V2(self.0 + rhs.0, self.1 + rhs.1)
    }
}
impl std::ops::AddAssign<V2> for V2 {
    fn add_assign(&mut self, rhs: V2) {
        self.0 += rhs.0;
        self.1 += rhs.1;
    }
}

#[derive(Debug, Clone)]
struct Momentum {
    pos: V2,
    vel: V2,
    bounds: V2,
}
impl Momentum {
    fn new(position: V2, velocity: V2, bounds: V2) -> Self {
        Momentum {
            pos: position,
            vel: velocity,
            bounds: bounds,
        }
    }
    fn apply_acceleration(&mut self, time: &Duration, acceleration: &V2) {
        let dt = f64_duration(time);
        let position =
            self.pos.clone() + self.vel.clone() * dt + acceleration.clone() * 0.5 * dt * dt;
        self.pos = position % self.bounds.clone();
        self.vel += acceleration.clone() * dt;
    }
    fn no_acceleration(&mut self, time: &Duration) {
        self.apply_acceleration(time, &V2::ZERO);
    }
    fn apply_impulse(&mut self, impulse: &V2) {
        self.vel += impulse.clone()
    }
    fn set_pos(&mut self, position: V2) {
        self.pos = position;
    }
    fn get_pos(&self) -> &V2 {
        &self.pos
    }
}

struct Ship<'a> {
    angle: f64,
    momentum: Momentum,
    thrust: bool,
    thrust_texture: Texture<'a>,
    inert_texture: Texture<'a>,
}
impl<'a> Ship<'a> {
    // pizxels
    const TEXTURE_SIZE: u32 = 32;
    // pixels per second per second.
    const ACCEL: f64 = 100.0;
    // radians per seocond
    const ANGULAR_ACCEL: f64 = 4.0;

    fn new(
        canvas: &mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>,
    ) -> Self {
        Ship {
            angle: 0.0,
            momentum: Momentum::new(V2(100.0, 100.0), V2(0.0, 0.0), V2(800.0, 600.0)),
            thrust: false,
            thrust_texture: Ship::draw_thrust_texture(canvas, texture_creator),
            inert_texture: Ship::draw_inert_texture(canvas, texture_creator),
        }
    }

    fn step(&mut self, duration: &Duration, thrust: bool, left: bool, right: bool) {
        self.thrust = thrust;
        let dt = f64_duration(duration);
        if left {
            self.angle -= Ship::ANGULAR_ACCEL * dt;
        }
        if right {
            self.angle += Ship::ANGULAR_ACCEL * dt;
        }
        let accel = if thrust {
            V2(
                self.angle.cos() * Ship::ACCEL,
                self.angle.sin() * Ship::ACCEL,
            )
        } else {
            V2::ZERO
        };
        self.momentum.apply_acceleration(duration, &accel);
    }

    fn draw(&self, canvas: &mut Canvas<Window>) {
        let centre: Point = self.momentum.get_pos().clone().into();
        let bounds = Rect::from_center(centre, 32, 32);

        let texture = if self.thrust {
            &self.thrust_texture
        } else {
            &self.inert_texture
        };

        canvas
            .copy_ex(
                texture,
                None,
                Some(bounds),
                self.angle * 180.0 / core::f64::consts::PI + 90.0,
                None,
                false,
                false,
            )
            .unwrap();
    }

    fn draw_thrust_texture(
        canvas: &mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>,
    ) -> Texture<'a> {
        let mut texture = texture_creator
            .create_texture_target(None, Ship::TEXTURE_SIZE, Ship::TEXTURE_SIZE)
            .expect("Could not create ship texture");

        canvas
            .with_texture_canvas(&mut texture, |texture_canvas| {
                texture_canvas.set_draw_color(BLACK);
                texture_canvas.clear();
                texture_canvas
                    .polygon(&[7, 16, 25, 16], &[32, 0, 32, 25], WHITE)
                    .unwrap();
                texture_canvas
                    .filled_polygon(&[12, 16, 20, 16], &[29, 32, 29, 25], WHITE)
                    .unwrap();
            })
            .expect("Could not draw ship texture");

        texture
    }

    fn draw_inert_texture(
        canvas: &mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>,
    ) -> Texture<'a> {
        let mut texture = texture_creator
            .create_texture_target(None, Ship::TEXTURE_SIZE, Ship::TEXTURE_SIZE)
            .expect("Could not create ship texture");

        canvas
            .with_texture_canvas(&mut texture, |texture_canvas| {
                texture_canvas.set_draw_color(BLACK);
                texture_canvas.clear();
                texture_canvas.set_draw_color(WHITE);
                texture_canvas
                    .polygon(&[7, 16, 25, 16], &[32, 0, 32, 25], WHITE)
                    .unwrap();
            })
            .expect("Could not draw ship texture");

        texture
    }
}

struct Asteroid<'a> {
    momentum: Momentum,
    radius: f64,
    texture:<'a>,
}
impl Asteroid<'a> {
    const TEXTURE_SIZE: u32 = 32;
    const INITIAL_RADIUS: f64 = 32.0;
    const MIN_RADIUS: f64 = 7.0;
    const VELOCITY_CHANGE: f64 = 5000.0;
    fn new (momentum: Momentum, radius: f64) -> Self{
        Asteroid {
            momentum: momentum,
            radius: radius,
        }
    }
    fn new_big_asteroid (momentum: Momentum) -> Self{
        Asteroid {
            momentum: momentum,
            radius: Asteroid::INITIAL_RADIUS,
        }
    }
    fn split (self) -> Option<(Self, Self)> {
        let new_radius = self.radius / 2.0;
        if new_radius < Asteroid::MIN_RADIUS {
            return None;
        }

        let dv = Asteroid::VELOCITY_CHANGE / new_radius;

        let mut rng = rand::thread_rng();
        let x1 = rng.gen_range(-dv, dv);
        let x2 = rng.gen_range(-dv, dv);
        let y1 = rng.gen_range(-dv, dv);
        let y2 = rng.gen_range(-dv, dv);

        let mut m1 = self.momentum.clone();
        let mut m2 = self.momentum;
        m1.apply_impulse(&V2(x1, y1));
        m2.apply_impulse(&V2(x2, y2));

        Some((Asteroid::new(m1, new_radius), Asteroid::new(m2, new_radius)))
    }
    fn step (&mut self, duration: &Duration) {
        self.momentum.no_acceleration(duration);
    }

    fn draw_texture (
        canvas: &mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>,
    ) -> Texture<'a> {
        let mut texture = texture_creator
            .create_texture_target(None, Asteroid::TEXTURE_SIZE, Asteroid::TEXTURE_SIZE)
            .expect("Could not create asteroid texture");
        
        canvas
            .with_texture_canvas(&mut texture, |texture_canvas| {
                texture_canvas.set_draw_color(BLACK);
                texture_canvas.clear();
                texture_canvas
                    .polygon(&[7, 16, 25, 16], &[32, 0, 32, 25], WHITE)
                    .unwrap();
            })
            .expect("Could not draw asteroid texture");

    }
}

pub fn main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("Asteroids", 800, 600)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let mut keys = Keys::new();
    let mut time = Instant::now();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut ship = Ship::new(&mut canvas, &texture_creator);
    loop {
        keys.with_events(&mut event_pump);
        if let Keys {
            quit: KeyStatus::Down,
            ..
        } = keys
        {
            break;
        }

        let dt = time.elapsed();
        time = Instant::now();
        canvas.set_draw_color(BLACK);
        canvas.clear();

        ship.step(&dt, keys.thrust.down(), keys.left.down(), keys.right.down());
        ship.draw(&mut canvas);

        canvas.present();

        std::thread::sleep(std::time::Duration::from_millis(1000 / 60));
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn key_status() {
        let mut key = KeyStatus::Down;
        key.step();
        assert_eq!(key, KeyStatus::Held);

        let mut key = KeyStatus::Held;
        key.step();
        assert_eq!(key, KeyStatus::Held);

        let mut key = KeyStatus::Up;
        key.step();
        assert_eq!(key, KeyStatus::Up);
    }
}