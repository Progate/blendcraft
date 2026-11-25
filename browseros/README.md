# BrowserOS branch

This branch adds `browseros/blendcraft`, a clean-room 3D creation application written in Rust for
BrowserOS. It is not Blender and does not contain Blender source code, assets, documentation text,
or file-format compatibility. The implementation was written from public, non-copylefted Craft
repository infrastructure and general 3D editor behavior.

The crate is a library (`blendcraft`) so `Progate/packages` can provide the BrowserOS windowserver
entry point in `packages/browser-blender/native`. The UI uses egui only; rendering is the crate's
CPU software projection/raster path and does not require wgpu, WebGPU, OpenGL, or a native window
system.

Current scope:

- scene/outliner with selectable objects
- cube, plane, UV sphere, and cylinder primitives
- object transforms, materials, visibility, duplicate/delete, undo/redo
- object mode and basic edit-mode vertex dragging
- orbit/pan/zoom camera and CPU shaded viewport
- `.blendcraft` JSON scene save/load
- Wavefront OBJ import/export for triangle meshes

License: MIT OR Apache-2.0, same as the fork base.
