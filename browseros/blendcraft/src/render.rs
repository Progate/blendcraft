use egui::{Color32, Pos2};

use crate::math::Vec3;
use crate::scene::{Document, SceneObject};

#[derive(Clone, Debug)]
pub struct ScreenTriangle {
    pub object_id: u64,
    pub triangle_index: usize,
    pub points: [Pos2; 3],
    pub depth: f32,
    pub vertex_depths: [f32; 3],
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
                vertex_depths: [za, zb, zc],
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

fn edge(a: Pos2, b: Pos2, point: Pos2) -> f32 {
    (point.x - a.x) * (b.y - a.y) - (point.y - a.y) * (b.x - a.x)
}

/// Rasterize the scene with a per-pixel depth buffer. The editor's scenes are small, so a simple
/// scan converter is fast enough and avoids the visible artifacts of triangle-level sorting when
/// objects overlap or intersect.
pub fn rasterize_scene(document: &Document, width: usize, height: usize) -> egui::ColorImage {
    let mut pixels = vec![Color32::TRANSPARENT; width * height];
    let mut depth_buffer = vec![f32::INFINITY; width * height];
    let triangles = render_scene(document, width as f32, height as f32);
    for triangle in triangles {
        let [a, b, c] = triangle.points;
        let area = edge(a, b, c);
        if area.abs() <= f32::EPSILON {
            continue;
        }
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(width as f32) as usize;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(height as f32) as usize;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let point = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let wa = edge(b, c, point) / area;
                let wb = edge(c, a, point) / area;
                let wc = edge(a, b, point) / area;
                if wa < -0.0001 || wb < -0.0001 || wc < -0.0001 {
                    continue;
                }
                let inverse_depth = wa / triangle.vertex_depths[0] + wb / triangle.vertex_depths[1] + wc / triangle.vertex_depths[2];
                if inverse_depth <= 0.0 {
                    continue;
                }
                let depth = 1.0 / inverse_depth;
                let index = y * width + x;
                if depth < depth_buffer[index] {
                    depth_buffer[index] = depth;
                    pixels[index] = triangle.color;
                }
            }
        }
    }
    egui::ColorImage::new([width, height], pixels)
}
