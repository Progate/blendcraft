use serde::{Deserialize, Serialize};

use crate::math::Vec3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mesh {
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub triangles: Vec<[usize; 3]>,
}

impl Mesh {
    pub fn new(vertices: Vec<Vec3>, triangles: Vec<[usize; 3]>) -> Self {
        let mut mesh = Self { vertices, normals: Vec::new(), triangles };
        mesh.recompute_normals();
        mesh
    }

    pub fn recompute_normals(&mut self) {
        self.normals = vec![Vec3::ZERO; self.vertices.len()];
        for triangle in &self.triangles {
            let [a, b, c] = *triangle;
            let (Some(&pa), Some(&pb), Some(&pc)) = (self.vertices.get(a), self.vertices.get(b), self.vertices.get(c)) else {
                continue;
            };
            let normal = (pb - pa).cross(pc - pa);
            self.normals[a] += normal;
            self.normals[b] += normal;
            self.normals[c] += normal;
        }
        for normal in &mut self.normals {
            *normal = normal.normalized();
        }
    }

    pub fn cube() -> Self {
        let v = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        let t = vec![[0, 2, 1], [0, 3, 2], [4, 5, 6], [4, 6, 7], [0, 1, 5], [0, 5, 4], [1, 2, 6], [1, 6, 5], [2, 3, 7], [2, 7, 6], [3, 0, 4], [3, 4, 7]];
        Self::new(v, t)
    }

    pub fn plane(size: f32) -> Self {
        let h = size / 2.0;
        Self::new(vec![Vec3::new(-h, 0.0, -h), Vec3::new(h, 0.0, -h), Vec3::new(h, 0.0, h), Vec3::new(-h, 0.0, h)], vec![[0, 1, 2], [0, 2, 3]])
    }

    pub fn uv_sphere(radius: f32, segments: usize, rings: usize) -> Self {
        let segments = segments.max(3);
        let rings = rings.max(2);
        let mut vertices = Vec::new();
        for ring in 0..=rings {
            let v = ring as f32 / rings as f32;
            let phi = v * std::f32::consts::PI;
            for segment in 0..segments {
                let u = segment as f32 / segments as f32;
                let theta = u * std::f32::consts::TAU;
                vertices.push(Vec3::new(radius * phi.sin() * theta.cos(), radius * phi.cos(), radius * phi.sin() * theta.sin()));
            }
        }
        let mut triangles = Vec::new();
        for ring in 0..rings {
            for segment in 0..segments {
                let next = (segment + 1) % segments;
                let a = ring * segments + segment;
                let b = ring * segments + next;
                let c = (ring + 1) * segments + next;
                let d = (ring + 1) * segments + segment;
                triangles.push([a, d, c]);
                triangles.push([a, c, b]);
            }
        }
        Self::new(vertices, triangles)
    }

    pub fn cylinder(radius: f32, depth: f32, segments: usize) -> Self {
        let segments = segments.max(3);
        let mut vertices = Vec::new();
        for y in [-depth / 2.0, depth / 2.0] {
            for segment in 0..segments {
                let theta = segment as f32 / segments as f32 * std::f32::consts::TAU;
                vertices.push(Vec3::new(radius * theta.cos(), y, radius * theta.sin()));
            }
        }
        let bottom_center = vertices.len();
        vertices.push(Vec3::new(0.0, -depth / 2.0, 0.0));
        let top_center = vertices.len();
        vertices.push(Vec3::new(0.0, depth / 2.0, 0.0));
        let mut triangles = Vec::new();
        for segment in 0..segments {
            let next = (segment + 1) % segments;
            let (b0, b1) = (segment, next);
            let (t0, t1) = (segments + segment, segments + next);
            triangles.push([b0, t1, t0]);
            triangles.push([b0, b1, t1]);
            triangles.push([bottom_center, b1, b0]);
            triangles.push([top_center, t0, t1]);
        }
        Self::new(vertices, triangles)
    }

    pub fn primitive(name: &str) -> Option<Self> {
        match name {
            "cube" => Some(Self::cube()),
            "plane" => Some(Self::plane(4.0)),
            "sphere" => Some(Self::uv_sphere(1.0, 24, 12)),
            "cylinder" => Some(Self::cylinder(1.0, 2.0, 24)),
            _ => None,
        }
    }

    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut iter = self.vertices.iter().copied();
        let first = iter.next()?;
        let mut low = first;
        let mut high = first;
        for vertex in iter {
            low = low.min(vertex);
            high = high.max(vertex);
        }
        Some((low, high))
    }
}
