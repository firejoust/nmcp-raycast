// src/raycast.rs
use crate::coords::WorldCoords;
use glam::DVec3; // Use DVec3 for f64 precision
use napi_derive::napi;

// Enum to represent block faces (matches prismarine-world convention)
#[napi]
#[derive(Debug, PartialEq)]
pub enum BlockFace {
    Bottom = 0, // -Y
    Top = 1,    // +Y
    North = 2,  // -Z
    South = 3,  // +Z
    West = 4,   // -X
    East = 5,   // +X
}

// Struct to represent the result returned to JavaScript
#[napi(object)]
#[derive(Debug)]
pub struct RaycastResult {
    pub position: WorldCoords, // Position of the intersected block
    pub face: u32,             // Numeric value of the BlockFace enum
    pub intersect_point: Vec3Arg, // Exact point of intersection
}

// Struct to receive Vec3 arguments from JavaScript
#[napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct Vec3Arg {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<Vec3Arg> for DVec3 {
    fn from(arg: Vec3Arg) -> Self {
        DVec3::new(arg.x, arg.y, arg.z)
    }
}

impl From<DVec3> for Vec3Arg {
    fn from(vec: DVec3) -> Self {
        Vec3Arg { x: vec.x, y: vec.y, z: vec.z }
    }
}

// --- Raycasting Algorithm (Amanatides & Woo variant) ---

pub struct RaycastIterator {
    pub origin: DVec3,
    pub direction: DVec3,
    pub max_distance_sq: f64,

    pub current_pos: WorldCoords,
    pub step: WorldCoords,
    pub t_max: DVec3,
    pub t_delta: DVec3,

    pub current_t: f64,
    pub just_started: bool,
    pub current_face: BlockFace,
}

impl RaycastIterator {
    pub fn new(origin: DVec3, direction: DVec3, max_distance: f64) -> Self {
        let current_pos = WorldCoords {
            x: origin.x.floor() as i32,
            y: origin.y.floor() as i32,
            z: origin.z.floor() as i32,
        };

        let step = WorldCoords {
            x: if direction.x > 0.0 { 1 } else { -1 },
            y: if direction.y > 0.0 { 1 } else { -1 },
            z: if direction.z > 0.0 { 1 } else { -1 },
        };

        // Avoid division by zero
        let t_delta = DVec3::new(
            if direction.x == 0.0 { f64::INFINITY } else { (1.0 / direction.x).abs() },
            if direction.y == 0.0 { f64::INFINITY } else { (1.0 / direction.y).abs() },
            if direction.z == 0.0 { f64::INFINITY } else { (1.0 / direction.z).abs() },
        );

        let t_max = DVec3::new(
            if direction.x == 0.0 { f64::INFINITY } else {
                let next_x = (current_pos.x as f64) + if direction.x > 0.0 { 1.0 } else { 0.0 };
                (next_x - origin.x) / direction.x
            },
            if direction.y == 0.0 { f64::INFINITY } else {
                let next_y = (current_pos.y as f64) + if direction.y > 0.0 { 1.0 } else { 0.0 };
                (next_y - origin.y) / direction.y
            },
            if direction.z == 0.0 { f64::INFINITY } else {
                let next_z = (current_pos.z as f64) + if direction.z > 0.0 { 1.0 } else { 0.0 };
                (next_z - origin.z) / direction.z
            },
        );

        RaycastIterator {
            origin,
            direction,
            max_distance_sq: max_distance * max_distance,
            current_pos,
            step,
            t_max,
            t_delta,
            current_t: 0.0,
            just_started: true,
            current_face: BlockFace::Bottom, // Initial arbitrary face
        }
    }

    pub fn next(&mut self) -> Option<(WorldCoords, BlockFace)> {
        if self.just_started {
            self.just_started = false;
            // Check if the starting block itself is the target
            if self.current_t * self.current_t * self.direction.length_squared() <= self.max_distance_sq {
                 // The initial face doesn't make much sense, maybe return a special value or calculate based on entry?
                 // For simplicity, let's just return the current block coords and an arbitrary face for the start.
                 // A more robust implementation might calculate the entry face if starting inside.
                return Some((self.current_pos, BlockFace::Bottom));
            } else {
                return None; // Started beyond max distance
            }
        }

        let face: BlockFace;
        if self.t_max.x < self.t_max.y {
            if self.t_max.x < self.t_max.z {
                self.current_t = self.t_max.x;
                self.current_pos.x += self.step.x;
                self.t_max.x += self.t_delta.x;
                face = if self.step.x > 0 { BlockFace::West } else { BlockFace::East };
            } else {
                self.current_t = self.t_max.z;
                self.current_pos.z += self.step.z;
                self.t_max.z += self.t_delta.z;
                face = if self.step.z > 0 { BlockFace::North } else { BlockFace::South };
            }
        } else {
            if self.t_max.y < self.t_max.z {
                self.current_t = self.t_max.y;
                self.current_pos.y += self.step.y;
                self.t_max.y += self.t_delta.y;
                face = if self.step.y > 0 { BlockFace::Bottom } else { BlockFace::Top };
            } else {
                self.current_t = self.t_max.z;
                self.current_pos.z += self.step.z;
                self.t_max.z += self.t_delta.z;
                face = if self.step.z > 0 { BlockFace::North } else { BlockFace::South };
            }
        }
        self.current_face = face;

        // Check distance using squared length to avoid sqrt
        if self.current_t * self.current_t * self.direction.length_squared() > self.max_distance_sq {
            None
        } else {
            Some((self.current_pos, face.clone()))
        }
    }

    // Calculates the exact intersection point given the current t value
    pub fn intersection_point(&self) -> DVec3 {
        self.origin + self.direction * self.current_t
    }
}

// --- AABB Intersection Test (Slab Method) ---
pub fn intersect_aabb(aabb_min: DVec3, aabb_max: DVec3, ray_origin: DVec3, ray_inv_dir: DVec3) -> Option<(f64, BlockFace)> {
    let tx1 = (aabb_min.x - ray_origin.x) * ray_inv_dir.x;
    let tx2 = (aabb_max.x - ray_origin.x) * ray_inv_dir.x;

    let mut tmin = tx1.min(tx2);
    let mut tmax = tx1.max(tx2);

    let ty1 = (aabb_min.y - ray_origin.y) * ray_inv_dir.y;
    let ty2 = (aabb_max.y - ray_origin.y) * ray_inv_dir.y;

    tmin = tmin.max(ty1.min(ty2));
    tmax = tmax.min(ty1.max(ty2));

    let tz1 = (aabb_min.z - ray_origin.z) * ray_inv_dir.z;
    let tz2 = (aabb_max.z - ray_origin.z) * ray_inv_dir.z;

    tmin = tmin.max(tz1.min(tz2));
    tmax = tmax.min(tz1.max(tz2));

    if tmax >= tmin && tmax >= 0.0 { // Ensure intersection is not behind the origin and tmin <= tmax
        // Determine the face of intersection based on which slab boundary was hit at tmin
        let mut face = BlockFace::Bottom; // Default, should be overwritten
        let epsilon = 1e-6; // Tolerance for float comparisons

        if (tmin - tx1).abs() < epsilon { face = BlockFace::West; }
        else if (tmin - tx2).abs() < epsilon { face = BlockFace::East; }
        else if (tmin - ty1).abs() < epsilon { face = BlockFace::Bottom; }
        else if (tmin - ty2).abs() < epsilon { face = BlockFace::Top; }
        else if (tmin - tz1).abs() < epsilon { face = BlockFace::North; }
        else if (tmin - tz2).abs() < epsilon { face = BlockFace::South; }

        Some((tmin, face))
    } else {
        None
    }
}