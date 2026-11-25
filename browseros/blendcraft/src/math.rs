use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const ONE: Self = Self::new(1.0, 1.0, 1.0);
    pub const UP: Self = Self::new(0.0, 1.0, 0.0);

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(self.y * other.z - self.z * other.y, self.z * other.x - self.x * other.z, self.x * other.y - self.y * other.x)
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length <= f32::EPSILON { Self::ZERO } else { self / length }
    }

    pub fn min(self, other: Self) -> Self {
        Self::new(self.x.min(other.x), self.y.min(other.y), self.z.min(other.z))
    }

    pub fn max(self, other: Self) -> Self {
        Self::new(self.x.max(other.x), self.y.max(other.y), self.z.max(other.z))
    }

    pub fn rotate(self, rotation: Self) -> Self {
        let (sx, cx) = rotation.x.sin_cos();
        let (sy, cy) = rotation.y.sin_cos();
        let (sz, cz) = rotation.z.sin_cos();
        let mut v = self;
        let (y, z) = (v.y * cx - v.z * sx, v.y * sx + v.z * cx);
        v.y = y;
        v.z = z;
        let (x, z) = (v.x * cy + v.z * sy, -v.x * sy + v.z * cy);
        v.x = x;
        v.z = z;
        let (x, y) = (v.x * cz - v.y * sz, v.x * sz + v.y * cz);
        v.x = x;
        v.y = y;
        v
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl std::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;
    fn mul(self, scalar: f32) -> Self {
        Self::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl std::ops::Mul<Vec3> for Vec3 {
    type Output = Self;
    fn mul(self, other: Vec3) -> Self {
        Self::new(self.x * other.x, self.y * other.y, self.z * other.z)
    }
}

impl std::ops::Div<f32> for Vec3 {
    type Output = Self;
    fn div(self, scalar: f32) -> Self {
        Self::new(self.x / scalar, self.y / scalar, self.z / scalar)
    }
}

impl std::ops::Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y, -self.z)
    }
}

pub fn vec3_ui(ui: &mut egui::Ui, label: &str, value: &mut Vec3, speed: f32) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(egui::DragValue::new(&mut value.x).speed(speed).prefix("X "));
        ui.add(egui::DragValue::new(&mut value.y).speed(speed).prefix("Y "));
        ui.add(egui::DragValue::new(&mut value.z).speed(speed).prefix("Z "));
    });
}

/// Undo the X-then-Y-then-Z Euler rotation used by scene objects.
pub fn inverse_rotate_euler(point: Vec3, rotation: Vec3) -> Vec3 {
    let mut v = point;
    let (sz, cz) = (-rotation.z).sin_cos();
    let (x, y) = (v.x * cz - v.y * sz, v.x * sz + v.y * cz);
    v.x = x;
    v.y = y;
    let (sy, cy) = (-rotation.y).sin_cos();
    let (x, z) = (v.x * cy + v.z * sy, -v.x * sy + v.z * cy);
    v.x = x;
    v.z = z;
    let (sx, cx) = (-rotation.x).sin_cos();
    let (y, z) = (v.y * cx - v.z * sx, v.y * sx + v.z * cx);
    v.y = y;
    v.z = z;
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_rotate_euler_restores_vector() {
        let point = Vec3::new(0.4, -1.2, 2.5);
        for rotation in [Vec3::new(0.2, -0.4, 0.8), Vec3::new(-1.1, 0.3, -0.2), Vec3::new(1.2, 1.0, -0.7)] {
            let restored = inverse_rotate_euler(point.rotate(rotation), rotation);
            assert!((restored.x - point.x).abs() < 0.0001);
            assert!((restored.y - point.y).abs() < 0.0001);
            assert!((restored.z - point.z).abs() < 0.0001);
        }
    }
}
