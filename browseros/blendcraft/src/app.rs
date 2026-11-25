use std::path::PathBuf;

use egui::{Color32, Key, PointerButton, RichText, Sense, Stroke};

use crate::math::{Vec3, vec3_ui};
use crate::mesh::Mesh;
use crate::render::{point_in_triangle, render_scene};
use crate::scene::{Document, PrimitiveKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Object,
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Grab,
    Rotate,
    Scale,
}

pub struct BlendCraftApp {
    pub document: Document,
    pub mode: Mode,
    pub tool: Tool,
    pub file_path: Option<PathBuf>,
    pub dirty: bool,
    pub quit_requested: bool,
    status: String,
    selected_vertex: Option<usize>,
    drag_start: Option<(egui::Pos2, Vec3)>,
    undo: Vec<Document>,
    redo: Vec<Document>,
}

impl Default for BlendCraftApp {
    fn default() -> Self {
        Self::new()
    }
}

impl BlendCraftApp {
    pub fn new() -> Self {
        Self {
            document: Document::default(),
            mode: Mode::Object,
            tool: Tool::Select,
            file_path: None,
            dirty: false,
            quit_requested: false,
            status: "Ready".into(),
            selected_vertex: None,
            drag_start: None,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    pub fn open_path(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let bytes = std::fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        if path.extension().is_some_and(|ext| ext == "obj") {
            let mesh = parse_obj(&bytes).map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("mesh.obj").to_string();
            self.push_undo();
            self.document.add_mesh(name, mesh);
        } else {
            let text = String::from_utf8(bytes).map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
            self.document = Document::from_json(&text).map_err(|error| format!("cannot parse {}: {error}", path.display()))?;
        }
        self.file_path = Some(path);
        self.dirty = false;
        self.undo.clear();
        self.redo.clear();
        self.status = "Opened".into();
        Ok(())
    }

    pub fn save(&mut self) -> Result<(), String> {
        let Some(path) = self.file_path.clone() else { return Err("no file name yet".into()) };
        self.save_as(path)
    }

    pub fn save_as(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let text = self.document.to_json().map_err(|error| error.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.file_path = Some(path);
        self.dirty = false;
        self.status = "Saved".into();
        Ok(())
    }

    pub fn export_obj(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let mut text = String::from("# BlendCraft OBJ export\n");
        let mut offset = 1usize;
        for object in self.document.objects.iter().filter(|object| object.visible) {
            text.push_str(&format!("o {}\n", object.name.replace(' ', "_")));
            for vertex in &object.mesh.vertices {
                let p = object.transform_point(*vertex);
                text.push_str(&format!("v {:.6} {:.6} {:.6}\n", p.x, p.y, p.z));
            }
            for triangle in &object.mesh.triangles {
                text.push_str(&format!("f {} {} {}\n", triangle[0] + offset, triangle[1] + offset, triangle[2] + offset));
            }
            offset += object.mesh.vertices.len();
        }
        std::fs::write(&path, text).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        self.status = format!("Exported {}", path.display());
        Ok(())
    }

    pub fn logic(&mut self, ctx: &egui::Context) {
        let (tab, delete, duplicate, grab, rotate, scale, undo, redo, escape) = ctx.input(|input| {
            (
                input.key_pressed(Key::Tab),
                input.key_pressed(Key::Delete) || input.key_pressed(Key::X),
                input.key_pressed(Key::D),
                input.key_pressed(Key::G),
                input.key_pressed(Key::R),
                input.key_pressed(Key::S),
                input.modifiers.command && input.key_pressed(Key::Z) && !input.modifiers.shift,
                (input.modifiers.command && input.key_pressed(Key::Y)) || (input.modifiers.command && input.modifiers.shift && input.key_pressed(Key::Z)),
                input.key_pressed(Key::Escape),
            )
        });
        if escape {
            self.tool = Tool::Select;
            self.drag_start = None;
            self.status = "Cancelled".into();
        }
        if tab {
            self.mode = if self.mode == Mode::Object { Mode::Edit } else { Mode::Object };
            self.selected_vertex = None;
            self.status = format!("{:?} mode", self.mode);
        }
        if grab {
            self.tool = Tool::Grab;
        }
        if rotate {
            self.tool = Tool::Rotate;
        }
        if scale {
            self.tool = Tool::Scale;
        }
        if delete && self.mode == Mode::Object {
            self.push_undo();
            if self.document.delete_selected() {
                self.dirty = true;
                self.status = "Object deleted".into();
            }
        }
        if duplicate && self.mode == Mode::Object {
            self.push_undo();
            if self.document.duplicate_selected().is_some() {
                self.dirty = true;
                self.status = "Object duplicated".into();
            }
        }
        if undo {
            self.undo_once();
        }
        if redo {
            self.redo_once();
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }

    pub fn close_requested(&mut self) -> bool {
        !self.dirty
    }

    fn push_undo(&mut self) {
        self.undo.push(self.document.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn undo_once(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.document, previous));
            self.dirty = true;
            self.status = "Undo".into();
        }
    }

    fn redo_once(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.document, next));
            self.dirty = true;
            self.status = "Redo".into();
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.menu(ui);
        egui::Panel::left("outliner").default_size(180.0).show(ui, |ui| self.outliner(ui));
        egui::Panel::right("properties").default_size(270.0).show(ui, |ui| self.properties(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} · {:?} · {:?} tool", self.status, self.mode, self.tool)).small());
                ui.separator();
                ui.label(RichText::new("Tab edit/object · G/R/S transform · MMB orbit · Shift+MMB pan · wheel zoom").small().weak());
            });
        });
        self.viewport(ui);
    }

    fn menu(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Scene").clicked() {
                    self.push_undo();
                    self.document = Document::default();
                    self.file_path = None;
                    self.dirty = true;
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    if let Err(error) = self.save() {
                        self.status = error
                    } else {
                        self.status = "Saved".into()
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Export OBJ…").clicked() {
                    let path = self.file_path.clone().unwrap_or_else(|| PathBuf::from("scene.blendcraft")).with_extension("obj");
                    if let Err(error) = self.export_obj(path) {
                        self.status = error
                    }
                    ui.close();
                }
                ui.separator();
                if ui.button("Quit").clicked() {
                    self.quit_requested = true;
                    ui.close();
                }
            });
            ui.menu_button("Add", |ui| {
                for kind in [PrimitiveKind::Cube, PrimitiveKind::Plane, PrimitiveKind::Sphere, PrimitiveKind::Cylinder] {
                    if ui.button(kind.label()).clicked() {
                        self.push_undo();
                        self.document.add(kind);
                        self.dirty = true;
                        self.status = format!("Added {}", kind.label());
                        ui.close();
                    }
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui.button("Undo").clicked() {
                    self.undo_once();
                    ui.close();
                }
                if ui.button("Redo").clicked() {
                    self.redo_once();
                    ui.close();
                }
                ui.separator();
                if ui.button("Duplicate").clicked() {
                    self.push_undo();
                    self.document.duplicate_selected();
                    self.dirty = true;
                    ui.close();
                }
                if ui.button("Delete").clicked() {
                    self.push_undo();
                    self.document.delete_selected();
                    self.dirty = true;
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                if ui.button("Frame All").clicked() {
                    self.document.camera.target = Vec3::ZERO;
                    self.document.camera.distance = 8.0;
                    ui.close();
                }
                if ui.button(if self.mode == Mode::Object { "Edit Mode" } else { "Object Mode" }).clicked() {
                    self.mode = if self.mode == Mode::Object { Mode::Edit } else { Mode::Object };
                    ui.close();
                }
            });
        });
    }

    fn outliner(&mut self, ui: &mut egui::Ui) {
        ui.heading("Outliner");
        ui.separator();
        let mut selected = self.document.selected;
        let mut visible_toggle = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for object in &self.document.objects {
                ui.horizontal(|ui| {
                    let is_selected = selected == Some(object.id);
                    if ui.selectable_label(is_selected, &object.name).clicked() {
                        selected = Some(object.id);
                    }
                    if ui.small_button(if object.visible { "👁" } else { "−" }).clicked() {
                        visible_toggle = Some(object.id);
                    }
                });
            }
        });
        if self.document.selected != selected {
            self.document.selected = selected;
        }
        if let Some(id) = visible_toggle {
            self.push_undo();
            if let Some(object) = self.document.object_mut(id) {
                object.visible = !object.visible;
                self.dirty = true;
            }
        }
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Properties");
        ui.separator();
        let Some(selected) = self.document.selected else {
            ui.label("No object selected");
            return;
        };
        let Some(object) = self.document.object_mut(selected) else { return };
        ui.horizontal(|ui| {
            ui.label("Name");
            if ui.text_edit_singleline(&mut object.name).changed() {
                self.dirty = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Type");
            ui.label(object.kind.label());
        });
        ui.add_space(6.0);
        ui.label(RichText::new("Transform").strong());
        if vec3_changed(ui, "Location", &mut object.translation, 0.02) {
            self.dirty = true;
        }
        let mut degrees = Vec3::new(object.rotation.x.to_degrees(), object.rotation.y.to_degrees(), object.rotation.z.to_degrees());
        if vec3_changed(ui, "Rotation°", &mut degrees, 0.5) {
            object.rotation = Vec3::new(degrees.x.to_radians(), degrees.y.to_radians(), degrees.z.to_radians());
            self.dirty = true;
        }
        if vec3_changed(ui, "Scale", &mut object.scale, 0.02) {
            self.dirty = true;
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Material").strong());
        let mut color = egui::Color32::from_rgb(
            (object.material.base_color[0] * 255.0) as u8,
            (object.material.base_color[1] * 255.0) as u8,
            (object.material.base_color[2] * 255.0) as u8,
        );
        ui.horizontal(|ui| {
            ui.label("Base color");
            if ui.color_edit_button_srgba(&mut color).changed() {
                object.material.base_color = [color.r() as f32 / 255.0, color.g() as f32 / 255.0, color.b() as f32 / 255.0];
                self.dirty = true;
            }
        });
        if ui.add(egui::Slider::new(&mut object.material.metallic, 0.0..=1.0).text("Metallic")).changed() {
            self.dirty = true;
        }
        if ui.add(egui::Slider::new(&mut object.material.roughness, 0.0..=1.0).text("Roughness")).changed() {
            self.dirty = true;
        }
        ui.add_space(6.0);
        ui.label(RichText::new("Mesh").strong());
        ui.label(format!("{} vertices · {} triangles", object.mesh.vertices.len(), object.mesh.triangles.len()));
        if self.mode == Mode::Edit {
            ui.label("Click near a vertex to select it; drag to move it in the view plane.");
            if let Some(vertex) = self.selected_vertex {
                ui.label(format!("Selected vertex #{vertex}"));
            }
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(38, 42, 48));
        self.draw_grid(&painter, rect);
        let width = rect.width().max(1.0);
        let height = rect.height().max(1.0);
        let triangles = render_scene(&self.document, width, height);
        for triangle in &triangles {
            let points = triangle.points.iter().map(|p| rect.min + p.to_vec2()).collect::<Vec<_>>();
            painter.add(egui::Shape::convex_polygon(points, triangle.color, Stroke::NONE));
        }
        if let Some(selected) = self.document.selected {
            for triangle in triangles.iter().filter(|triangle| triangle.object_id == selected) {
                let points = triangle.points.iter().map(|p| rect.min + p.to_vec2()).collect::<Vec<_>>();
                painter.add(egui::Shape::closed_line(points, Stroke::new(1.0, Color32::from_rgb(255, 166, 64))));
            }
        }
        self.viewport_input(ui, &response, rect);
        painter.text(
            rect.left_top() + egui::vec2(8.0, 8.0),
            egui::Align2::LEFT_TOP,
            format!("{:?} mode", self.mode),
            egui::FontId::proportional(13.0),
            Color32::from_gray(210),
        );
    }

    fn draw_grid(&mut self, painter: &egui::Painter, rect: egui::Rect) {
        let width = rect.width().max(1.0);
        let height = rect.height().max(1.0);
        let stroke = Stroke::new(1.0, Color32::from_rgb(58, 62, 70));
        for i in -10..=10 {
            for (a, b, color) in [
                (Vec3::new(i as f32, 0.0, -10.0), Vec3::new(i as f32, 0.0, 10.0), stroke),
                (Vec3::new(-10.0, 0.0, i as f32), Vec3::new(10.0, 0.0, i as f32), stroke),
            ] {
                if let (Some((pa, _)), Some((pb, _))) = (self.document.camera.project(a, width, height), self.document.camera.project(b, width, height)) {
                    painter.line_segment([rect.min + pa.to_vec2(), rect.min + pb.to_vec2()], color);
                }
            }
        }
        for (a, b, color) in [
            (Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Color32::from_rgb(200, 70, 70)),
            (Vec3::ZERO, Vec3::new(0.0, 2.0, 0.0), Color32::from_rgb(80, 190, 90)),
            (Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0), Color32::from_rgb(80, 120, 220)),
        ] {
            if let (Some((pa, _)), Some((pb, _))) = (self.document.camera.project(a, width, height), self.document.camera.project(b, width, height)) {
                painter.line_segment([rect.min + pa.to_vec2(), rect.min + pb.to_vec2()], Stroke::new(2.0, color));
            }
        }
    }

    fn viewport_input(&mut self, ui: &mut egui::Ui, response: &egui::Response, rect: egui::Rect) {
        let width = rect.width().max(1.0);
        let height = rect.height().max(1.0);
        let scroll = ui.ctx().input(|input| input.smooth_scroll_delta.y);
        if response.hovered() && scroll.abs() > 0.0 {
            self.document.camera.distance = (self.document.camera.distance * (1.0 - scroll * 0.0015)).clamp(1.2, 80.0);
        }
        let delta = response.drag_delta();
        if response.dragged_by(PointerButton::Middle) {
            if ui.ctx().input(|input| input.modifiers.shift) {
                self.document.camera.pan(delta.x, delta.y);
            } else {
                self.document.camera.yaw -= delta.x * 0.008;
                self.document.camera.pitch = (self.document.camera.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
            }
        }
        if response.dragged_by(PointerButton::Secondary) {
            self.document.camera.yaw -= delta.x * 0.008;
            self.document.camera.pitch = (self.document.camera.pitch + delta.y * 0.008).clamp(-1.45, 1.45);
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            let local = pos - rect.min;
            if self.mode == Mode::Edit {
                self.pick_vertex(local, width, height);
            } else {
                self.pick_object(local, width, height);
            }
        }
        if response.dragged_by(PointerButton::Primary) {
            if self.mode == Mode::Edit {
                self.drag_vertex(delta, width, height);
            } else if self.tool != Tool::Select {
                self.drag_object(delta, width, height);
            }
        }
        if response.drag_stopped() {
            self.drag_start = None;
        }
    }

    fn pick_object(&mut self, local: egui::Vec2, width: f32, height: f32) {
        let point = egui::Pos2::new(local.x, local.y);
        let triangles = render_scene(&self.document, width, height);
        if let Some(hit) = triangles.iter().rev().find(|triangle| point_in_triangle(point, triangle.points)) {
            self.document.selected = Some(hit.object_id);
            self.status = "Object selected".into();
        }
    }

    fn pick_vertex(&mut self, local: egui::Vec2, width: f32, height: f32) {
        let Some(id) = self.document.selected else { return };
        let Some(object) = self.document.object(id) else { return };
        let mut best = None;
        let mut best_distance = 12.0f32;
        for (index, vertex) in object.mesh.vertices.iter().enumerate() {
            let world = object.transform_point(*vertex);
            if let Some((screen, _)) = object_transform_project(&self.document, object, world, width, height) {
                let distance = screen.distance(egui::pos2(local.x, local.y));
                if distance < best_distance {
                    best_distance = distance;
                    best = Some(index);
                }
            }
        }
        self.selected_vertex = best;
        if best.is_some() {
            self.status = "Vertex selected".into();
        }
    }

    fn drag_vertex(&mut self, delta: egui::Vec2, _width: f32, height: f32) {
        let (Some(id), Some(index)) = (self.document.selected, self.selected_vertex) else { return };
        if self.drag_start.is_none() {
            if let Some(object) = self.document.object(id).and_then(|object| object.mesh.vertices.get(index)).copied() {
                self.drag_start = Some((egui::pos2(0.0, 0.0), object));
                self.push_undo();
            }
        }
        let Some((_, start_vertex)) = self.drag_start else { return };
        let camera = self.document.camera;
        let (_, right, up, forward) = camera.basis();
        let focal = (height * 0.5) / (camera.fov_y_degrees.to_radians() * 0.5).tan();
        let depth = (camera.eye() - self.document.object(id).map(|object| object.transform_point(start_vertex)).unwrap_or(Vec3::ZERO)).dot(forward).max(0.5);
        let world_delta = right * (delta.x * depth / focal) + up * (-delta.y * depth / focal);
        if let Some(object) = self.document.object_mut(id)
            && let Some(vertex) = object.mesh.vertices.get_mut(index)
        {
            let inverse_rotation = Vec3::new(-object.rotation.x, -object.rotation.y, -object.rotation.z);
            let local_delta = world_delta.rotate(inverse_rotation);
            vertex.x += local_delta.x / object.scale.x.max(0.001);
            vertex.y += local_delta.y / object.scale.y.max(0.001);
            vertex.z += local_delta.z / object.scale.z.max(0.001);
            object.mesh.recompute_normals();
            self.dirty = true;
        }
    }

    fn drag_object(&mut self, delta: egui::Vec2, _width: f32, _height: f32) {
        if self.drag_start.is_none() {
            self.push_undo();
            self.drag_start = Some((egui::pos2(0.0, 0.0), Vec3::ZERO));
        }
        let Some(id) = self.document.selected else { return };
        let camera = self.document.camera;
        let Some(object) = self.document.object_mut(id) else { return };
        match self.tool {
            Tool::Select => {}
            Tool::Grab => {
                let (_, right, up, _) = camera.basis();
                let scale = camera.distance * 0.0022;
                object.translation += right * (delta.x * scale) + up * (-delta.y * scale);
            }
            Tool::Rotate => {
                object.rotation.y += delta.x * 0.012;
                object.rotation.x += delta.y * 0.012;
            }
            Tool::Scale => {
                let amount = 1.0 + (delta.x - delta.y) * 0.008;
                object.scale = object.scale * amount.max(0.02);
            }
        }
        self.dirty = true;
    }
}

fn object_transform_project(document: &Document, _object: &crate::scene::SceneObject, world: Vec3, width: f32, height: f32) -> Option<(egui::Pos2, f32)> {
    document.camera.project(world, width, height)
}

fn vec3_changed(ui: &mut egui::Ui, label: &str, value: &mut Vec3, speed: f32) -> bool {
    let before = *value;
    vec3_ui(ui, label, value, speed);
    before != *value
}

pub fn parse_obj(bytes: &[u8]) -> Result<Mesh, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| error.to_string())?;
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let values: Vec<f32> = parts.filter_map(|part| part.parse().ok()).collect();
                if values.len() < 3 {
                    return Err("invalid vertex line".into());
                }
                positions.push(Vec3::new(values[0], values[1], values[2]));
            }
            Some("f") => {
                let mut indices = Vec::new();
                for part in parts {
                    let first = part.split('/').next().ok_or("invalid face")?;
                    let index: isize = first.parse().map_err(|_| "invalid face index")?;
                    let resolved = if index < 0 { positions.len() as isize + index + 1 } else { index };
                    if resolved <= 0 {
                        return Err("invalid face index".into());
                    }
                    indices.push((resolved - 1) as usize);
                }
                if indices.len() < 3 {
                    return Err("face has fewer than 3 vertices".into());
                }
                for i in 1..indices.len() - 1 {
                    triangles.push([indices[0], indices[i], indices[i + 1]]);
                }
            }
            _ => {}
        }
    }
    if positions.is_empty() || triangles.is_empty() {
        return Err("OBJ has no triangles".into());
    }
    Ok(Mesh::new(positions, triangles))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obj_roundtrip_mesh() {
        let mesh = parse_obj(b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.triangles.len(), 1);
    }
}
