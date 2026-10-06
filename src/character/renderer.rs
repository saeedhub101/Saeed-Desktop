use std::path::Path;

use slint::{Rgba8Pixel, SharedPixelBuffer};

use super::MotionIntent;

#[derive(Debug, Clone, Copy, Default)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn add(self, other: Self) -> Self { Self { x: self.x + other.x, y: self.y + other.y, z: self.z + other.z } }
    fn sub(self, other: Self) -> Self { Self { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z } }
    fn mul(self, scalar: f32) -> Self { Self { x: self.x * scalar, y: self.y * scalar, z: self.z * scalar } }
    fn dot(self, other: Self) -> f32 { self.x * other.x + self.y * other.y + self.z * other.z }
    fn cross(self, other: Self) -> Self {
        Self {
            x: self.y * other.z - self.z * other.y,
            y: self.z * other.x - self.x * other.z,
            z: self.x * other.y - self.y * other.x,
        }
    }
    fn length(self) -> f32 { self.dot(self).sqrt() }
    fn normalize(self) -> Self {
        let len = self.length();
        if len > 0.0001 { self.mul(1.0 / len) } else { Self { x: 0.0, y: 0.0, z: 1.0 } }
    }
}

#[derive(Debug, Clone, Copy)]
struct Triangle {
    a: Vec3,
    b: Vec3,
    c: Vec3,
    color: [u8; 4],
}

#[derive(Debug, Clone, Copy)]
struct Rotation {
    yaw: f32,
    pitch: f32,
    roll: f32,
}

impl Rotation {
    fn apply(self, p: Vec3) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let (sr, cr) = self.roll.sin_cos();

        let x1 = cy * p.x + sy * p.z;
        let z1 = -sy * p.x + cy * p.z;
        let y2 = cp * p.y - sp * z1;
        let z2 = sp * p.y + cp * z1;
        Vec3 {
            x: cr * x1 - sr * y2,
            y: sr * x1 + cr * y2,
            z: z2,
        }
    }
}

/// A real GLB runtime that loads mesh geometry and rasterizes it into a Slint image.
/// It is intentionally event-driven: rendering happens only when the owner requests
/// a new frame, so hidden characters consume no render-loop CPU.
pub struct GlbCharacterRenderer {
    triangles: Vec<Triangle>,
    width: u32,
    height: u32,
    center: Vec3,
    scale: f32,
}

impl GlbCharacterRenderer {
    pub fn from_path(path: impl AsRef<Path>, width: u32, height: u32) -> Result<Self, String> {
        let (document, buffers, _) = gltf::import(path.as_ref())
            .map_err(|error| format!("Could not load GLB '{}': {error}", path.as_ref().display()))?;

        let mut triangles = Vec::new();
        let scene = document
            .default_scene()
            .or_else(|| document.scenes().next())
            .ok_or_else(|| "Character GLB contains no scene.".to_string())?;

        for node in scene.nodes() {
            collect_node(&node, &buffers, Mat4::identity(), &mut triangles)?;
        }

        if triangles.is_empty() {
            return Err("Character GLB contains no renderable triangles.".to_string());
        }

        let mut min = Vec3 { x: f32::INFINITY, y: f32::INFINITY, z: f32::INFINITY };
        let mut max = Vec3 { x: f32::NEG_INFINITY, y: f32::NEG_INFINITY, z: f32::NEG_INFINITY };
        for triangle in &triangles {
            for p in [triangle.a, triangle.b, triangle.c] {
                min.x = min.x.min(p.x); min.y = min.y.min(p.y); min.z = min.z.min(p.z);
                max.x = max.x.max(p.x); max.y = max.y.max(p.y); max.z = max.z.max(p.z);
            }
        }

        let center = min.add(max).mul(0.5);
        let extent = max.sub(min);
        let largest = extent.x.max(extent.y).max(extent.z).max(0.001);
        let scale = 2.0 / largest;

        Ok(Self { triangles, width: width.max(64), height: height.max(64), center, scale })
    }

    pub fn render(&self, motion: MotionIntent) -> SharedPixelBuffer<Rgba8Pixel> {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut pixels = SharedPixelBuffer::<Rgba8Pixel>::new(self.width, self.height);
        let slice = pixels.make_mut_slice();
        for pixel in slice.iter_mut() {
            *pixel = Rgba8Pixel { r: 0, g: 0, b: 0, a: 0 };
        }

        let mut depth = vec![f32::INFINITY; width * height];
        let rotation = Rotation {
            yaw: motion.yaw,
            pitch: motion.pitch,
            roll: motion.roll,
        };

        for triangle in &self.triangles {
            let a = rotation.apply(triangle.a.sub(self.center).mul(self.scale));
            let b = rotation.apply(triangle.b.sub(self.center).mul(self.scale));
            let c = rotation.apply(triangle.c.sub(self.center).mul(self.scale));

            let normal = b.sub(a).cross(c.sub(a)).normalize();
            let light = normal.dot(Vec3 { x: -0.35, y: 0.65, z: 0.75 }.normalize()).max(0.15);
            let color = [
                (triangle.color[0] as f32 * light) as u8,
                (triangle.color[1] as f32 * light) as u8,
                (triangle.color[2] as f32 * light) as u8,
                triangle.color[3],
            ];

            let project = |p: Vec3| -> (f32, f32, f32) {
                let camera_z = p.z + 3.2;
                let perspective = 2.2 / camera_z.max(0.25);
                (
                    self.width as f32 * 0.5 + p.x * self.width as f32 * 0.42 * perspective,
                    self.height as f32 * 0.52 - p.y * self.height as f32 * 0.42 * perspective,
                    camera_z,
                )
            };

            let pa = project(a);
            let pb = project(b);
            let pc = project(c);

            rasterize_triangle(&mut pixels, &mut depth, pa, pb, pc, color);
        }

        pixels
    }
}

#[derive(Clone, Copy)]
struct Mat4([[f32; 4]; 4]);

impl Mat4 {
    fn identity() -> Self {
        Self([
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
    }

    fn mul(self, other: Self) -> Self {
        let mut out = [[0.0; 4]; 4];
        for r in 0..4 {
            for c in 0..4 {
                out[r][c] = (0..4).map(|k| self.0[r][k] * other.0[k][c]).sum();
            }
        }
        Self(out)
    }

    fn transform(self, p: Vec3) -> Vec3 {
        let x = self.0[0][0] * p.x + self.0[0][1] * p.y + self.0[0][2] * p.z + self.0[0][3];
        let y = self.0[1][0] * p.x + self.0[1][1] * p.y + self.0[1][2] * p.z + self.0[1][3];
        let z = self.0[2][0] * p.x + self.0[2][1] * p.y + self.0[2][2] * p.z + self.0[2][3];
        Vec3 { x, y, z }
    }

    fn from_gltf(transform: gltf::scene::Transform) -> Self {
        let matrix = transform.matrix();
        let mut out = [[0.0; 4]; 4];
        // glTF exposes matrices in column-major form; the rasterizer uses row-major.
        for column in 0..4 {
            for row in 0..4 {
                out[row][column] = matrix[column][row];
            }
        }
        Self(out)
    }
}

fn collect_node(
    node: &gltf::Node<'_>,
    buffers: &[gltf::buffer::Data],
    parent: Mat4,
    triangles: &mut Vec<Triangle>,
) -> Result<(), String> {
    let world = parent.mul(Mat4::from_gltf(node.transform()));

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            let positions: Vec<Vec3> = reader
                .read_positions()
                .ok_or_else(|| "GLB mesh primitive has no POSITION attribute.".to_string())?
                .map(|p| Vec3 { x: p[0], y: p[1], z: p[2] })
                .collect();

            let indices: Vec<u32> = match reader.read_indices() {
                Some(iter) => iter.into_u32().collect(),
                None => (0..positions.len() as u32).collect(),
            };

            let base = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();

            let color = [
                (base[0].clamp(0.0, 1.0) * 255.0) as u8,
                (base[1].clamp(0.0, 1.0) * 255.0) as u8,
                (base[2].clamp(0.0, 1.0) * 255.0) as u8,
                (base[3].clamp(0.0, 1.0) * 255.0) as u8,
            ];

            for index in indices.chunks_exact(3) {
                let a = world.transform(positions[index[0] as usize]);
                let b = world.transform(positions[index[1] as usize]);
                let c = world.transform(positions[index[2] as usize]);
                triangles.push(Triangle { a, b, c, color });
            }
        }
    }

    for child in node.children() {
        collect_node(&child, buffers, world, triangles)?;
    }

    Ok(())
}

fn rasterize_triangle(
    pixels: &mut SharedPixelBuffer<Rgba8Pixel>,
    depth: &mut [f32],
    a: (f32, f32, f32),
    b: (f32, f32, f32),
    c: (f32, f32, f32),
    color: [u8; 4],
) {
    let min_x = a.0.min(b.0).min(c.0).floor().max(0.0) as i32;
    let max_x = a.0.max(b.0).max(c.0).ceil().min(pixels.width() as f32 - 1.0) as i32;
    let min_y = a.1.min(b.1).min(c.1).floor().max(0.0) as i32;
    let max_y = a.1.max(b.1).max(c.1).ceil().min(pixels.height() as f32 - 1.0) as i32;

    let area = edge(a.0, a.1, b.0, b.1, c.0, c.1);
    if area.abs() < 0.0001 {
        return;
    }

    let out = pixels.make_mut_slice();
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = edge(b.0, b.1, c.0, c.1, px, py) / area;
            let w1 = edge(c.0, c.1, a.0, a.1, px, py) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }

            let z = w0 * a.2 + w1 * b.2 + w2 * c.2;
            let index = y as usize * pixels.width() as usize + x as usize;
            if z < depth[index] {
                depth[index] = z;
                out[index] = Rgba8Pixel { r: color[0], g: color[1], b: color[2], a: color[3] };
            }
        }
    }
}

fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (px - ax) * (by - ay) - (py - ay) * (bx - ax)
}
