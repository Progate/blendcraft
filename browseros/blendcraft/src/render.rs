use egui::{Color32, Pos2};

use crate::math::Vec3;
use crate::scene::{Document, SceneObject};

#[derive(Clone, Debug)]
pub struct ScreenTriangle {
    pub object_id: u64,
    pub triangle_index: usize,
    pub points: [Pos2; 3],
    pub depth: f32,
    pub color: Color32,
}

fn shade(object: &SceneObject, world_normal: Vec3) -> Color32 {
    let light = Vec3::new(-0.45, 0.8, 0.38).normalized();
    let diffuse = world_normal.dot(light).max(0.0);
    let ambient = 0.28;
    let roughness = object.material.roughness.clamp(0.05, 1.0);
    let intensity = (ambient + diffuse * (0.9 - roughness * 0.25)).min(1.0);
    let [r, g, b] = object.material.base_color;
    let [er, eg, eb] = object.material.emission;
    let channel = |base: f32, emission: f32| ((base * intensity + emission).clamp(0.0, 1.0) * 255.0) as u8;
    Color32::from_rgb(channel(r, er), channel(g, eg), channel(b, eb))
}

/// Project the scene into screen-space triangles. Hidden-surface removal uses the painter's algorithm;
/// the small meshes this editor starts with favour correctness and simplicity over a z-buffer.
pub fn render_scene(document: &Document, width: f32, height: f32) -> Vec<ScreenTriangle> {
    let mut output = Vec::new();
    for object in &document.objects {
        if !object.visible {
            continue;
        }
        let world: Vec<Vec3> = object.mesh.vertices.iter().copied().map(|point| object.transform_point(point)).collect();
        let projected: Vec<Option<(Pos2, f32)>> = world.iter().copied().map(|point| document.camera.project(point, width, height)).collect();
        for (triangle_index, triangle) in object.mesh.triangles.iter().enumerate() {
            let [ia, ib, ic] = *triangle;
            let (Some(pa), Some(pb), Some(pc)) = (projected.get(ia), projected.get(ib), projected.get(ic)) else { continue };
            let (Some((pa, za)), Some((pb, zb)), Some((pc, zc))) = (*pa, *pb, *pc) else { continue };
            let normal = (world[ib] - world[ia]).cross(world[ic] - world[ia]).normalized();
            output.push(ScreenTriangle {
                object_id: object.id,
                triangle_index,
                points: [pa, pb, pc],
                depth: (za + zb + zc) / 3.0,
                color: shade(object, normal),
            });
        }
    }
    output.sort_by(|a, b| b.depth.partial_cmp(&a.depth).unwrap_or(std::cmp::Ordering::Equal));
    output
}

pub fn point_in_triangle(point: Pos2, triangle: [Pos2; 3]) -> bool {
    let [a, b, c] = triangle;
    let sign = |p1: Pos2, p2: Pos2, p3: Pos2| (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y);
    let d1 = sign(point, a, b);
    let d2 = sign(point, b, c);
    let d3 = sign(point, c, a);
    let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(negative && positive)
}
