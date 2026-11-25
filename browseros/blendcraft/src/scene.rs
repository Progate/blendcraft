use serde::{Deserialize, Serialize};

use crate::math::Vec3;
use crate::mesh::Mesh;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrimitiveKind {
    Cube,
    Plane,
    Sphere,
    Cylinder,
    Imported,
}

impl PrimitiveKind {
    pub fn mesh(self) -> Option<Mesh> {
        match self {
            Self::Cube => Mesh::primitive("cube"),
            Self::Plane => Mesh::primitive("plane"),
            Self::Sphere => Mesh::primitive("sphere"),
            Self::Cylinder => Mesh::primitive("cylinder"),
            Self::Imported => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Cube => "Cube",
            Self::Plane => "Plane",
            Self::Sphere => "UV Sphere",
            Self::Cylinder => "Cylinder",
            Self::Imported => "Imported Mesh",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Material {
    pub base_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emission: [f32; 3],
}

impl Default for Material {
    fn default() -> Self {
        Self { base_color: [0.62, 0.68, 0.78], metallic: 0.0, roughness: 0.55, emission: [0.0, 0.0, 0.0] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: u64,
    pub name: String,
    pub kind: PrimitiveKind,
    pub mesh: Mesh,
    pub translation: Vec3,
    /// Euler angles in radians, applied X then Y then Z.
    pub rotation: Vec3,
    pub scale: Vec3,
    pub material: Material,
    pub visible: bool,
}

impl SceneObject {
    pub fn new(id: u64, kind: PrimitiveKind, mesh: Mesh) -> Self {
        Self {
            id,
            name: format!("{}.{:03}", kind.label(), id),
            kind,
            mesh,
            translation: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
            material: Material::default(),
            visible: true,
        }
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        (point * self.scale).rotate(self.rotation) + self.translation
    }

    pub fn transform_direction(&self, direction: Vec3) -> Vec3 {
        direction.rotate(self.rotation).normalized()
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Camera {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y_degrees: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { target: Vec3::ZERO, distance: 8.0, yaw: -0.7, pitch: 0.45, fov_y_degrees: 45.0 }
    }
}

impl Camera {
    pub fn eye(&self) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        self.target + Vec3::new(self.distance * cp * sy, self.distance * sp, self.distance * cp * cy)
    }

    pub fn basis(&self) -> (Vec3, Vec3, Vec3, Vec3) {
        let eye = self.eye();
        let forward = (self.target - eye).normalized();
        let right = forward.cross(Vec3::UP).normalized();
        let up = right.cross(forward).normalized();
        (eye, right, up, forward)
    }

    pub fn project(&self, point: Vec3, width: f32, height: f32) -> Option<(egui::Pos2, f32)> {
        let (eye, right, up, forward) = self.basis();
        let local = point - eye;
        let z = local.dot(forward);
        if z <= 0.05 {
            return None;
        }
        let focal = (height * 0.5) / (self.fov_y_degrees.to_radians() * 0.5).tan();
        let x = local.dot(right);
        let y = local.dot(up);
        Some((egui::pos2(width * 0.5 + x * focal / z, height * 0.5 - y * focal / z), z))
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        let (_, right, up, _) = self.basis();
        self.target += right * (-dx * self.distance * 0.0018) + up * (dy * self.distance * 0.0018);
    }
}

fn default_format_version() -> u32 {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Document {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    pub objects: Vec<SceneObject>,
    pub selected: Option<u64>,
    pub next_id: u64,
    pub camera: Camera,
}

impl Default for Document {
    fn default() -> Self {
        let mut document = Self { format_version: default_format_version(), objects: Vec::new(), selected: None, next_id: 1, camera: Camera::default() };
        document.add(PrimitiveKind::Cube);
        document
    }
}

impl Document {
    pub fn add(&mut self, kind: PrimitiveKind) -> Option<u64> {
        let mesh = kind.mesh()?;
        let id = self.next_id;
        self.next_id += 1;
        self.objects.push(SceneObject::new(id, kind, mesh));
        self.selected = Some(id);
        Some(id)
    }

    pub fn add_mesh(&mut self, name: String, mesh: Mesh) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mut object = SceneObject::new(id, PrimitiveKind::Imported, mesh);
        object.name = name;
        self.objects.push(object);
        self.selected = Some(id);
        id
    }

    pub fn object(&self, id: u64) -> Option<&SceneObject> {
        self.objects.iter().find(|object| object.id == id)
    }

    pub fn object_mut(&mut self, id: u64) -> Option<&mut SceneObject> {
        self.objects.iter_mut().find(|object| object.id == id)
    }

    pub fn selected_mut(&mut self) -> Option<&mut SceneObject> {
        let id = self.selected?;
        self.object_mut(id)
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected.take() else { return false };
        let before = self.objects.len();
        self.objects.retain(|object| object.id != id);
        before != self.objects.len()
    }

    pub fn duplicate_selected(&mut self) -> Option<u64> {
        let source = self.selected.and_then(|id| self.object(id)).cloned()?;
        let id = self.next_id;
        self.next_id += 1;
        let mut copy = source;
        copy.id = id;
        copy.name = format!("{} copy", copy.name.trim_end_matches(" copy"));
        copy.translation.x += 1.0;
        self.objects.push(copy);
        self.selected = Some(id);
        Some(id)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}
