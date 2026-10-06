use std::collections::HashMap;
use std::path::Path;

use slint::{Rgba8Pixel, SharedPixelBuffer};

use super::MotionIntent;

#[derive(Debug, Clone, Copy, Default)]
struct Vec3 { x: f32, y: f32, z: f32 }

impl Vec3 {
    fn add(self, o: Self) -> Self { Self { x: self.x + o.x, y: self.y + o.y, z: self.z + o.z } }
    fn sub(self, o: Self) -> Self { Self { x: self.x - o.x, y: self.y - o.y, z: self.z - o.z } }
    fn mul(self, s: f32) -> Self { Self { x: self.x*s, y: self.y*s, z: self.z*s } }
    fn dot(self, o: Self) -> f32 { self.x*o.x + self.y*o.y + self.z*o.z }
    fn cross(self, o: Self) -> Self { Self { x:self.y*o.z-self.z*o.y, y:self.z*o.x-self.x*o.z, z:self.x*o.y-self.y*o.x } }
    fn length(self) -> f32 { self.dot(self).sqrt() }
    fn normalize(self) -> Self {
        let l=self.length();
        if l>0.0001 { self.mul(1.0/l) } else { Self{x:0.0,y:0.0,z:1.0} }
    }
}

#[derive(Debug, Clone, Copy)]
struct Vertex {
    position: Vec3,
    joints: [u16; 4],
    weights: [f32; 4],
}

#[derive(Debug, Clone, Copy)]
struct Triangle {
    vertices: [Vertex; 3],
    color: [u8; 4],
    skin: Option<usize>,
}

#[derive(Debug, Clone)]
struct Joint {
    name: String,
    inverse_bind: Mat4,
    base_world: Mat4,
}

#[derive(Debug, Clone)]
struct SkinPalette { joints: Vec<Joint> }

#[derive(Debug, Clone, Copy)]
struct Rotation { yaw: f32, pitch: f32, roll: f32 }

impl Rotation {
    fn apply(self, p: Vec3) -> Vec3 {
        let (sy,cy)=self.yaw.sin_cos();
        let (sp,cp)=self.pitch.sin_cos();
        let (sr,cr)=self.roll.sin_cos();
        let x1=cy*p.x+sy*p.z;
        let z1=-sy*p.x+cy*p.z;
        let y2=cp*p.y-sp*z1;
        let z2=sp*p.y+cp*z1;
        Vec3{x:cr*x1-sr*y2,y:sr*x1+cr*y2,z:z2}
    }
}

/// GLB mesh renderer. It consumes MotionIntent only; it owns no character state,
/// no timers, and no animation controller. Skinned primitives use glTF JOINTS_0,
/// WEIGHTS_0 and inverse bind matrices for procedural bone motion.
pub struct GlbCharacterRenderer {
    triangles: Vec<Triangle>,
    palettes: Vec<SkinPalette>,
    width: u32,
    height: u32,
    center: Vec3,
    scale: f32,
    pub rigged: bool,
    pub bone_names: Vec<String>,
    procedural_fallback: bool,
}

impl GlbCharacterRenderer {
    pub fn from_path(path: impl AsRef<Path>, width: u32, height: u32) -> Result<Self, String> {
        let (document,buffers,_)=gltf::import(path.as_ref())
            .map_err(|e| format!("Could not load GLB '{}': {e}",path.as_ref().display()))?;
        let scene=document.default_scene().or_else(||document.scenes().next())
            .ok_or_else(||"Character GLB contains no scene.".to_string())?;

        let mut worlds=HashMap::new();
        for node in scene.nodes() { collect_worlds(&node,Mat4::identity(),&mut worlds); }

        let mut palettes=Vec::new();
        let mut palette_by_skin=HashMap::new();
        for skin in document.skins() {
            let reader=skin.reader(|b|Some(&buffers[b.index()]));
            let inverse: Vec<Mat4>=reader.read_inverse_bind_matrices()
                .map(|it|it.map(Mat4::from_gltf_array).collect())
                .unwrap_or_else(||vec![Mat4::identity();skin.joints().count()]);
            let joints=skin.joints().enumerate().map(|(i,node)|Joint{
                node_index:node.index(),
                name:node.name().unwrap_or("").to_string(),
                inverse_bind:inverse.get(i).copied().unwrap_or_else(Mat4::identity),
                base_world:worlds.get(&node.index()).copied().unwrap_or_else(Mat4::identity),
            }).collect::<Vec<_>>();
            palette_by_skin.insert(skin.index(),palettes.len());
            palettes.push(SkinPalette{joints});
        }

        let mut triangles=Vec::new();
        for node in scene.nodes() {
            collect_node(&node,&buffers,Mat4::identity(),&palette_by_skin,&mut triangles)?;
        }
        if triangles.is_empty() { return Err("Character GLB contains no renderable triangles.".into()); }

        let mut min=Vec3{x:f32::INFINITY,y:f32::INFINITY,z:f32::INFINITY};
        let mut max=Vec3{x:f32::NEG_INFINITY,y:f32::NEG_INFINITY,z:f32::NEG_INFINITY};
        for t in &triangles { for v in t.vertices { for p in [v.position] {
            min.x=min.x.min(p.x);min.y=min.y.min(p.y);min.z=min.z.min(p.z);
            max.x=max.x.max(p.x);max.y=max.y.max(p.y);max.z=max.z.max(p.z);
        }}}
        let center=min.add(max).mul(0.5);
        let e=max.sub(min);
        let scale=2.0/e.x.max(e.y).max(e.z).max(0.001);
        let mut bone_names=Vec::new();
        for p in &palettes { for j in &p.joints { if !j.name.is_empty() && !bone_names.contains(&j.name) { bone_names.push(j.name.clone()); } } }
        let rigged=!palettes.is_empty() && triangles.iter().any(|t|t.skin.is_some());

        Ok(Self{triangles,palettes,width:width.max(64),height:height.max(64),center,scale,rigged,bone_names,procedural_fallback:false})
    }

    /// Built-in humanoid fallback used when no external GLB is available. It keeps
    /// the application fully usable while preserving the same renderer boundary.
    pub fn procedural(width: u32, height: u32) -> Self {
        let mut triangles = Vec::new();
        cube(&mut triangles, Vec3{x:0.0,y:0.25,z:0.0}, Vec3{x:0.55,y:0.85,z:0.30}, [92,145,220,255]);
        cube(&mut triangles, Vec3{x:0.0,y:0.95,z:0.0}, Vec3{x:0.38,y:0.38,z:0.38}, [225,185,145,255]);
        cube(&mut triangles, Vec3{x:-0.48,y:0.25,z:0.0}, Vec3{x:0.18,y:0.65,z:0.18}, [92,145,220,255]);
        cube(&mut triangles, Vec3{x:0.48,y:0.25,z:0.0}, Vec3{x:0.18,y:0.65,z:0.18}, [92,145,220,255]);
        cube(&mut triangles, Vec3{x:-0.18,y:-0.48,z:0.0}, Vec3{x:0.20,y:0.70,z:0.22}, [65,80,110,255]);
        cube(&mut triangles, Vec3{x:0.18,y:-0.48,z:0.0}, Vec3{x:0.20,y:0.70,z:0.22}, [65,80,110,255]);
        let min=Vec3{x:-0.65,y:-0.83,z:-0.22};
        let max=Vec3{x:0.65,y:1.15,z:0.22};
        let center=min.add(max).mul(0.5);
        let e=max.sub(min);
        let scale=2.0/e.x.max(e.y).max(e.z).max(0.001);
        Self{triangles,palettes:Vec::new(),width:width.max(64),height:height.max(64),center,scale,rigged:false,bone_names:Vec::new(),procedural_fallback:true}
    }

    pub fn render(&self,motion:MotionIntent)->SharedPixelBuffer<Rgba8Pixel>{
        let mut pixels=SharedPixelBuffer::<Rgba8Pixel>::new(self.width,self.height);
        for p in pixels.make_mut_slice().iter_mut(){*p=Rgba8Pixel{r:0,g:0,b:0,a:0};}
        let mut depth=vec![f32::INFINITY;(self.width*self.height) as usize];
        let root=Rotation{yaw:motion.yaw,pitch:motion.pitch,roll:motion.roll};

        for t in &self.triangles {
            let mut pts=[Vec3::default();3];
            for (i,v) in t.vertices.iter().enumerate() {
                let mut p=if let Some(si)=t.skin { skin_vertex(v,&self.palettes[si],motion) } else { v.position };
                if self.procedural_fallback { p=procedural_pose(p,motion); }
                pts[i]=root.apply(p.sub(self.center).mul(self.scale));
            }
            let normal=pts[1].sub(pts[0]).cross(pts[2].sub(pts[0])).normalize();
            let light=normal.dot(Vec3{x:-0.35,y:0.65,z:0.75}.normalize()).max(0.15);
            let color=[(t.color[0] as f32*light) as u8,(t.color[1] as f32*light) as u8,(t.color[2] as f32*light) as u8,t.color[3]];
            let project=|p:Vec3| -> (f32,f32,f32) {
                let z=p.z+3.2;let perspective=2.2/z.max(0.25);
                (self.width as f32*0.5+p.x*self.width as f32*0.42*perspective,
                 self.height as f32*0.52-p.y*self.height as f32*0.42*perspective,z)
            };
            rasterize_triangle(&mut pixels,&mut depth,project(pts[0]),project(pts[1]),project(pts[2]),color);
        }
        pixels
    }
}


fn cube(triangles:&mut Vec<Triangle>, center:Vec3, size:Vec3, color:[u8;4]){
    let h=size.mul(0.5);
    let p=[
        Vec3{x:center.x-h.x,y:center.y-h.y,z:center.z-h.z}, Vec3{x:center.x+h.x,y:center.y-h.y,z:center.z-h.z},
        Vec3{x:center.x+h.x,y:center.y+h.y,z:center.z-h.z}, Vec3{x:center.x-h.x,y:center.y+h.y,z:center.z-h.z},
        Vec3{x:center.x-h.x,y:center.y-h.y,z:center.z+h.z}, Vec3{x:center.x+h.x,y:center.y-h.y,z:center.z+h.z},
        Vec3{x:center.x+h.x,y:center.y+h.y,z:center.z+h.z}, Vec3{x:center.x-h.x,y:center.y+h.y,z:center.z+h.z}
    ];
    let faces=[[0,1,2],[0,2,3],[1,5,6],[1,6,2],[5,4,7],[5,7,6],[4,0,3],[4,3,7],[3,2,6],[3,6,7],[4,5,1],[4,1,0]];
    for f in faces { let v=|i| Vertex{position:p[i],joints:[0;4],weights:[1.0,0.0,0.0,0.0]}; triangles.push(Triangle{vertices:[v(f[0]),v(f[1]),v(f[2])],color,skin:None}); }
}

fn procedural_pose(mut p:Vec3,motion:MotionIntent)->Vec3{
    let arm=motion.arm_wave;
    if p.x.abs()>0.32 && p.y>0.0 {
        let side=if p.x>0.0 {1.0}else{-1.0};
        let pivot=Vec3{x:side*0.30,y:0.52,z:0.0};
        let a=arm*side;
        let (s,c)=a.sin_cos();
        let x=p.x-pivot.x; let y=p.y-pivot.y;
        p.x=pivot.x+x*c-y*s; p.y=pivot.y+x*s+y*c;
    }
    p
}

fn skin_vertex(v:&Vertex,palette:&SkinPalette,motion:MotionIntent)->Vec3{
    let mut out=Vec3::default();
    let mut total=0.0;
    for i in 0..4 {
        let w=v.weights[i];
        if w<=0.00001 {continue;}
        let Some(j)=palette.joints.get(v.joints[i] as usize) else {continue;};
        let local=joint_motion(j,motion).mul(j.inverse_bind).transform(v.position);
        out=out.add(local.mul(w));total+=w;
    }
    if total>0.00001 {out.mul(1.0/total)} else {v.position}
}

fn joint_motion(j:&Joint,motion:MotionIntent)->Mat4{
    let n=j.name.to_ascii_lowercase();
    let is_arm=n.contains("upperarm")||n.contains("forearm")||n.contains("hand");
    if !is_arm || motion.arm_wave.abs()<0.0001 {return j.base_world;}
    let right=n.contains("right");
    let angle=motion.arm_wave*(if right {1.0}else{-1.0});
    let axis=if n.contains("forearm") {1}else{0};
    let r=if axis==0 {Mat4::rotation_z(angle)} else {Mat4::rotation_x(-angle*0.55)};
    j.base_world.mul(r)
}

fn collect_worlds(node:&gltf::Node<'_>,parent:Mat4,out:&mut HashMap<usize,Mat4>){
    let world=parent.mul(Mat4::from_gltf(node.transform()));
    out.insert(node.index(),world);
    for child in node.children(){collect_worlds(&child,world,out);}
}

fn collect_node(
    node:&gltf::Node<'_>,buffers:&[gltf::buffer::Data],parent:Mat4,
    palette_by_skin:&HashMap<usize,usize>,triangles:&mut Vec<Triangle>
)->Result<(),String>{
    let world=parent.mul(Mat4::from_gltf(node.transform()));
    if let Some(mesh)=node.mesh(){
        let skin_index=node.skin().map(|s|s.index()).and_then(|i|palette_by_skin.get(&i).copied());
        for primitive in mesh.primitives(){
            let reader=primitive.reader(|b|Some(&buffers[b.index()]));
            let positions:Vec<Vec3>=reader.read_positions().ok_or_else(||"GLB primitive has no POSITION.".to_string())?
                .map(|p|Vec3{x:p[0],y:p[1],z:p[2]}).collect();
            let joints:Option<Vec<[u16;4]>>=reader.read_joints(0).map(|j|j.into_u16().collect());
            let weights:Option<Vec<[f32;4]>>=reader.read_weights(0).map(|w|w.into_f32().collect());
            let indices:Vec<u32>=reader.read_indices().map(|i|i.into_u32().collect()).unwrap_or_else(||(0..positions.len() as u32).collect());
            let base=primitive.material().pbr_metallic_roughness().base_color_factor();
            let color=[(base[0]*255.0) as u8,(base[1]*255.0) as u8,(base[2]*255.0) as u8,(base[3]*255.0) as u8];
            for tri in indices.chunks_exact(3){
                let mut verts=[Vertex{position:Vec3::default(),joints:[0;4],weights:[1.0,0.0,0.0,0.0]};3];
                for k in 0..3{
                    let idx=tri[k] as usize;
                    verts[k]=Vertex{
                        position:if skin_index.is_some() { positions[idx] } else { world.transform(positions[idx]) },
                        joints:joints.as_ref().and_then(|v|v.get(idx)).copied().unwrap_or([0;4]),
                        weights:weights.as_ref().and_then(|v|v.get(idx)).copied().unwrap_or([1.0,0.0,0.0,0.0]),
                    };
                }
                triangles.push(Triangle{vertices:verts,color,skin:skin_index});
            }
        }
    }
    for child in node.children(){collect_node(&child,buffers,world,palette_by_skin,triangles)?;}
    Ok(())
}

#[derive(Clone,Copy,Debug)]
struct Mat4([[f32;4];4]);
impl Mat4{
    fn identity()->Self{Self([[1.0,0.0,0.0,0.0],[0.0,1.0,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]])}
    fn mul(self,o:Self)->Self{let mut out=[[0.0;4];4];for r in 0..4{for c in 0..4{for k in 0..4{out[r][c]+=self.0[r][k]*o.0[k][c];}}}Self(out)}
    fn transform(self,p:Vec3)->Vec3{Vec3{x:self.0[0][0]*p.x+self.0[0][1]*p.y+self.0[0][2]*p.z+self.0[0][3],y:self.0[1][0]*p.x+self.0[1][1]*p.y+self.0[1][2]*p.z+self.0[1][3],z:self.0[2][0]*p.x+self.0[2][1]*p.y+self.0[2][2]*p.z+self.0[2][3]}}
    fn from_gltf(t:gltf::scene::Transform)->Self{let m=t.matrix();let mut o=[[0.0;4];4];for c in 0..4{for r in 0..4{o[r][c]=m[c][r];}}Self(o)}
    fn from_gltf_array(m:[[f32;4];4])->Self{let mut o=[[0.0;4];4];for c in 0..4{for r in 0..4{o[r][c]=m[c][r];}}Self(o)}
    fn rotation_x(a:f32)->Self{let(s,c)=a.sin_cos();Self([[1.0,0.0,0.0,0.0],[0.0,c,-s,0.0],[0.0,s,c,0.0],[0.0,0.0,0.0,1.0]])}
    fn rotation_z(a:f32)->Self{let(s,c)=a.sin_cos();Self([[c,-s,0.0,0.0],[s,c,0.0,0.0],[0.0,0.0,1.0,0.0],[0.0,0.0,0.0,1.0]])}
}

fn rasterize_triangle(pixels:&mut SharedPixelBuffer<Rgba8Pixel>,depth:&mut[f32],a:(f32,f32,f32),b:(f32,f32,f32),c:(f32,f32,f32),color:[u8;4]){
    let min_x=a.0.min(b.0).min(c.0).floor().max(0.0) as i32;
    let max_x=a.0.max(b.0).max(c.0).ceil().min(pixels.width() as f32-1.0) as i32;
    let min_y=a.1.min(b.1).min(c.1).floor().max(0.0) as i32;
    let max_y=a.1.max(b.1).max(c.1).ceil().min(pixels.height() as f32-1.0) as i32;
    let area=edge(a.0,a.1,b.0,b.1,c.0,c.1);if area.abs()<0.0001{return;}
    let width=pixels.width() as usize;let out=pixels.make_mut_slice();
    for y in min_y..=max_y{for x in min_x..=max_x{
        let px=x as f32+0.5;let py=y as f32+0.5;
        let w0=edge(b.0,b.1,c.0,c.1,px,py)/area;let w1=edge(c.0,c.1,a.0,a.1,px,py)/area;let w2=1.0-w0-w1;
        if w0<0.0||w1<0.0||w2<0.0{continue;}
        let z=w0*a.2+w1*b.2+w2*c.2;let i=y as usize*width+x as usize;
        if z<depth[i]{depth[i]=z;out[i]=Rgba8Pixel{r:color[0],g:color[1],b:color[2],a:color[3]};}
    }}
}
fn edge(ax:f32,ay:f32,bx:f32,by:f32,px:f32,py:f32)->f32{(px-ax)*(by-ay)-(py-ay)*(bx-ax)}
