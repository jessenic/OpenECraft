//! 3D export of the panel layout for MCAD / PLM / BIM: the enclosures with their
//! components (body or frame, walls, side panels, door, mounting plate, gland plate,
//! plinth), accessories, rails, ducts, busbars and devices (the parts' 3D
//! models or boxes) with the drives of the machines on the floor as glTF 2.0 (JSON with embedded buffer, metres, y up) and – in
//! millimetres, z up – as STEP AP214 (faceted solids with colours), IGES 5.3
//! (manifold solid B-rep objects with planar faces), IFC4 (distribution boards with
//! triangulated elements), JT (tri-strip meshes) and – in metres – Parasolid XT
//! (faceted solid bodies).

mod ifc;
mod iges;

pub use ifc::ifc;
pub use iges::iges;

use crate::model::Project;
use crate::panel::{self, Enclosure, ItemKind, Role};
use crate::parts::vec3::{box_triangles, cross, norm, sub};
use std::collections::HashMap;
use std::fmt::Write;

type Tri = [[f32; 3]; 3];

/// One solid of the scene, in millimetres: x right, y up, z towards the door.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    /// Device tag or what it is ("Mounting plate", "Rail", …).
    pub name: String,
    pub color: [u8; 3],
    /// Shown see-through (door, walls of a room).
    pub transparent: bool,
    pub triangles: Vec<Tri>,
}

/// An enclosure with its solids and its offset (enclosures are placed side by side).
#[derive(Clone, Debug, PartialEq)]
pub struct EnclosureScene {
    pub name: String,
    pub offset: [f32; 3],
    pub solids: Vec<Solid>,
}

fn color(kind: ItemKind) -> [u8; 3] {
    match kind {
        ItemKind::Rail => [205, 208, 212],
        ItemKind::Duct => [150, 154, 160],
        ItemKind::Busbar => [200, 120, 60],
        ItemKind::Cutout => [40, 40, 44],
        ItemKind::Device => [170, 186, 210],
        ItemKind::Accessory => [95, 100, 110],
        ItemKind::MountingPlate => [185, 189, 195],
    }
}

/// Colour of an enclosure component.
pub fn component_color(role: Role) -> [u8; 3] {
    match role {
        Role::MountingPlate => [222, 218, 205],
        Role::Plinth => [60, 62, 66],
        Role::Body => [150, 155, 162],
        Role::GlandPlate => [175, 178, 184],
        _ => [200, 204, 210],
    }
}

fn kind_name(kind: ItemKind) -> &'static str {
    match kind {
        ItemKind::Rail => "Mounting rail",
        ItemKind::Duct => "Cable duct",
        ItemKind::Busbar => "Busbar",
        ItemKind::Cutout => "Cutout",
        ItemKind::Device => "Device",
        ItemKind::Accessory => "Accessory",
        ItemKind::MountingPlate => "Mounting plate",
    }
}

/// The solids of an enclosure (the coordinates of the 3D view).
pub fn enclosure_solids(prj: &Project, e: &Enclosure) -> Vec<Solid> {
    let h = e.height;
    let up = |q: [f32; 3]| [q[0], h - q[1], q[2]];
    // Boxes in enclosure coordinates (y down) to y up.
    let up_box = |(a, b): ([f32; 3], [f32; 3])| box_triangles(up([a[0], b[1], a[2]]), up([b[0], a[1], b[2]]));
    let mut out = Vec::new();
    let mut solid = |name: &str, color: [u8; 3], transparent: bool, triangles: Vec<Tri>| {
        out.push(Solid { name: name.to_string(), color, transparent, triangles });
    };
    for c in panel::components(e).into_iter().filter(|c| !c.boxes.is_empty()) {
        let name = if c.part.is_empty() { format!("{} {}", e.name, kind_role(c.role)) } else { format!("{} {} {}", e.name, kind_role(c.role), c.part) };
        let tris = c.boxes.into_iter().flat_map(up_box).collect();
        solid(&name, component_color(c.role), matches!(c.role, Role::Door | Role::RoomWall(_)), tris);
    }
    for it in e.items.iter().filter(|i| i.kind != ItemKind::Cutout) {
        // A busbar system: its copper bars and their insulating holders apart.
        if it.kind == ItemKind::Busbar {
            for (color, name) in [(panel::busbars::COPPER, "Busbar"), (panel::busbars::HOLDER, "Busbar holder")] {
                let tris: Vec<Tri> = panel::busbars::mesh(prj, it).into_iter().filter(|t| t.1 == Some(color)).map(|t| t.0).collect();
                solid(name, color, false, panel::place_triangles(e, it, tris).into_iter().map(|t| t.map(up)).collect());
            }
            continue;
        }
        let name = if it.tag.is_empty() { kind_name(it.kind) } else { it.tag.as_str() };
        let tris = match panel::device_mesh(prj, it) {
            Some(tris) => panel::place_triangles(e, it, tris).into_iter().map(|t| t.map(up)).collect(),
            None => {
                let (b0, b1) = panel::item_box(e, it);
                box_triangles(up([b0[0], b1[1], b0[2]]), up([b1[0], b0[1], b1[2]]))
            }
        };
        solid(name, color(it.kind), false, tris);
    }
    // The drives of the machines on the floor (shafts, couplings, belts, gear cases).
    for d in panel::drives(prj, e) {
        let name = e.items.iter().find(|i| i.id == d.machine).map_or_else(|| "Drive".to_string(), |i| format!("{} drive", i.tag));
        solid(&name, [112, 116, 124], false, d.triangles.into_iter().map(|(t, _)| t.map(up)).collect());
    }
    out
}

/// Name of a component in the exported scene (machine data, not translated).
fn kind_role(role: Role) -> &'static str {
    match role {
        Role::Body => "enclosure",
        Role::Door => "door",
        Role::Wall(panel::Face::Left) => "left side panel",
        Role::Wall(panel::Face::Right) => "right side panel",
        Role::Wall(panel::Face::Rear) => "rear wall",
        Role::Wall(panel::Face::Roof) => "roof",
        Role::Wall(_) => "bottom",
        Role::GlandPlate => "gland plate",
        Role::MountingPlate => "mounting plate",
        Role::Plinth => "plinth",
        Role::BayingKit => "baying kit",
        Role::RackRails => "19-inch mounting rails",

        Role::RoomWall(panel::Face::Roof) => "ceiling",
        Role::RoomWall(panel::Face::Inner(..)) => "inner wall",
        Role::RoomWall(_) => "wall",
    }
}

/// All enclosures of the project side by side: bayed ones in their line-up, all
/// standing on the floor (y 0).
pub fn scene(prj: &Project) -> Vec<EnclosureScene> {
    let pos = panel::positions(&prj.layout);
    prj.layout
        .enclosures
        .iter()
        .zip(pos)
        .map(|(e, o)| EnclosureScene { name: e.name.clone(), offset: [o[0], e.build.plinth, o[2]], solids: enclosure_solids(prj, e) })
        .collect()
}

/// The triangles of a solid in exchange coordinates: view (x right, y up, z to the
/// door) to z up – (x, -z, y) – plus the enclosure offset.
fn z_up(es: &EnclosureScene, solid: &Solid) -> Vec<Tri> {
    let tf = |v: [f32; 3]| [v[0] + es.offset[0], -(v[2] + es.offset[2]), v[1] + es.offset[1]];
    solid.triangles.iter().map(|t| t.map(tf)).collect()
}

/// Unit normal of a triangle; `None` if it is degenerate.
fn normal(t: &Tri) -> Option<[f32; 3]> {
    let n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
    (!n.iter().all(|c| c.abs() < 1e-9) && !n.iter().any(|c| c.is_nan())).then_some(n)
}

/// The scene as JT: a group per enclosure with a tri-strip set shape per solid.
pub fn jt(prj: &Project) -> Vec<u8> {
    jt_of(if prj.info.name.is_empty() { "Panel layout" } else { &prj.info.name }, &scene(prj))
}

/// Scenes as JT: a group per scene with a tri-strip set shape per solid.
pub fn jt_of(title: &str, scenes: &[EnclosureScene]) -> Vec<u8> {
    let tris: Vec<Vec<Vec<Tri>>> = scenes.iter().map(|es| es.solids.iter().map(|s| z_up(es, s)).collect()).collect();
    let groups: Vec<openecraft_jt::Group> = scenes
        .iter()
        .zip(&tris)
        .map(|(es, tris)| openecraft_jt::Group {
            name: &es.name,
            bodies: es.solids.iter().zip(tris).map(|(s, t)| openecraft_jt::Body { name: &s.name, color: s.color, transparent: s.transparent, triangles: t }).collect(),
        })
        .collect();
    openecraft_jt::write_jt(title, &groups)
}

/// The scene as Parasolid XT text (`.x_t`): every closed part of a solid a faceted
/// solid body, in metres, z up (see `openecraft_xt`).
pub fn parasolid(prj: &Project) -> String {
    let scenes = scene(prj);
    let tris: Vec<Vec<Tri>> = scenes.iter().flat_map(|es| es.solids.iter().map(|s| z_up(es, s))).collect();
    let meshes: Vec<&[Tri]> = tris.iter().map(Vec::as_slice).collect();
    openecraft_xt::write_xt(&meshes)
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// The scene as glTF 2.0 (JSON, buffer embedded as data URI): a node per enclosure
/// with a child node and mesh per solid, flat normals, one material per colour.
pub fn gltf(prj: &Project) -> String {
    gltf_of(&prj.info.name, &scene(prj))
}

/// Scenes as glTF 2.0: a node per scene with a child node and mesh per solid.
pub fn gltf_of(title: &str, scenes: &[EnclosureScene]) -> String {
    let mut bin: Vec<u8> = Vec::new();
    let (mut views, mut accessors, mut meshes, mut nodes, mut materials) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut material_of: HashMap<([u8; 3], bool), usize> = HashMap::new();
    let mut roots = Vec::new();
    for es in scenes {
        let mut children = Vec::new();
        for s in es.solids.iter().filter(|s| !s.triangles.is_empty()) {
            let mut pos: Vec<f32> = Vec::with_capacity(s.triangles.len() * 9);
            let mut nrm: Vec<f32> = Vec::with_capacity(s.triangles.len() * 9);
            let (mut min, mut max) = ([f32::MAX; 3], [f32::MIN; 3]);
            for t in &s.triangles {
                let n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
                for v in t {
                    let m = v.map(|c| c / 1000.0);
                    for k in 0..3 {
                        min[k] = min[k].min(m[k]);
                        max[k] = max[k].max(m[k]);
                    }
                    pos.extend(m);
                    nrm.extend(n);
                }
            }
            let count = s.triangles.len() * 3;
            for (data, with_bounds) in [(&pos, true), (&nrm, false)] {
                let offset = bin.len();
                bin.extend(data.iter().flat_map(|f| f.to_le_bytes()));
                views.push(format!(r#"{{"buffer":0,"byteOffset":{offset},"byteLength":{},"target":34962}}"#, data.len() * 4));
                let bounds = if with_bounds { format!(r#","min":[{},{},{}],"max":[{},{},{}]"#, min[0], min[1], min[2], max[0], max[1], max[2]) } else { String::new() };
                accessors.push(format!(r#"{{"bufferView":{},"componentType":5126,"count":{count},"type":"VEC3"{bounds}}}"#, views.len() - 1));
            }
            let key = (s.color, s.transparent);
            let material = *material_of.entry(key).or_insert_with(|| {
                let [r, g, b] = s.color.map(|c| c as f32 / 255.0);
                let (alpha, mode) = if s.transparent { (0.25, r#","alphaMode":"BLEND","doubleSided":true"#) } else { (1.0, "") };
                materials.push(format!(r#"{{"pbrMetallicRoughness":{{"baseColorFactor":[{r:.4},{g:.4},{b:.4},{alpha}],"metallicFactor":0.1,"roughnessFactor":0.7}}{mode}}}"#));
                materials.len() - 1
            });
            let a = accessors.len();
            meshes.push(format!(r#"{{"name":{},"primitives":[{{"attributes":{{"POSITION":{},"NORMAL":{}}},"material":{material}}}]}}"#, json_str(&s.name), a - 2, a - 1));
            nodes.push(format!(r#"{{"name":{},"mesh":{}}}"#, json_str(&s.name), meshes.len() - 1));
            children.push(nodes.len() - 1);
        }
        let [x, y, z] = es.offset.map(|c| c / 1000.0);
        let list = children.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",");
        nodes.push(format!(r#"{{"name":{},"translation":[{x},{y},{z}],"children":[{list}]}}"#, json_str(&es.name)));
        roots.push(nodes.len() - 1);
    }
    let roots = roots.iter().map(|r| r.to_string()).collect::<Vec<_>>().join(",");
    let mut out = String::from("{");
    let _ = write!(out, r#""asset":{{"version":"2.0","generator":"OpenECraft"}},"scene":0,"scenes":[{{"name":{},"nodes":[{roots}]}}],"#, json_str(title));
    let _ = write!(out, r#""nodes":[{}],"meshes":[{}],"materials":[{}],"#, nodes.join(","), meshes.join(","), materials.join(","));
    let _ = write!(out, r#""accessors":[{}],"bufferViews":[{}],"#, accessors.join(","), views.join(","));
    let _ = write!(out, r#""buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}]}}"#, bin.len(), crate::document::base64_encode(&bin));
    out
}

/// STEP real: always with a decimal point (`12.`, `0.5`).
fn real(v: f32) -> String {
    let v = if v.abs() < 1e-6 { 0.0 } else { v };
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0');
    s.to_string()
}

fn step_text(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii() && c != '\'' && c != '\\' { c } else { '_' }).collect()
}

struct Step {
    out: String,
    next: usize,
}

impl Step {
    fn add(&mut self, entity: &str) -> usize {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.out, "#{id}={entity};");
        id
    }
}

/// The scene as STEP AP214: one product with a faceted solid per item (planar
/// triangles), coloured; millimetres, x right, y into the enclosure, z up.
pub fn step(prj: &Project, file_name: &str) -> String {
    let name = step_text(if prj.info.name.is_empty() { "Panel layout" } else { &prj.info.name });
    let mut s = Step { out: String::new(), next: 1 };
    let ctx = s.add("APPLICATION_CONTEXT('automotive design')");
    s.add(&format!("APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{ctx})"));
    let pctx = s.add(&format!("PRODUCT_CONTEXT('',#{ctx},'mechanical')"));
    let product = s.add(&format!("PRODUCT('{name}','{name}','OpenECraft panel layout',(#{pctx}))"));
    s.add(&format!("PRODUCT_RELATED_PRODUCT_CATEGORY('part',$,(#{product}))"));
    let pdf = s.add(&format!("PRODUCT_DEFINITION_FORMATION('','',#{product})"));
    let pdc = s.add(&format!("PRODUCT_DEFINITION_CONTEXT('part definition',#{ctx},'design')"));
    let pd = s.add(&format!("PRODUCT_DEFINITION('design','',#{pdf},#{pdc})"));
    let pds = s.add(&format!("PRODUCT_DEFINITION_SHAPE('','',#{pd})"));
    let length = s.add("(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.))");
    let angle = s.add("(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.))");
    let solid_angle = s.add("(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT())");
    let unc = s.add(&format!("UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE(1.E-03),#{length},'distance_accuracy_value','')"));
    let gctx = s.add(&format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle}))REPRESENTATION_CONTEXT('',''))"
    ));
    let origin = s.add("CARTESIAN_POINT('',(0.,0.,0.))");
    let z = s.add("DIRECTION('',(0.,0.,1.))");
    let x = s.add("DIRECTION('',(1.,0.,0.))");
    let placement = s.add(&format!("AXIS2_PLACEMENT_3D('',#{origin},#{z},#{x})"));
    let mut items = vec![placement];
    let mut styled = Vec::new();
    let mut styles: HashMap<[u8; 3], usize> = HashMap::new();
    for es in scene(prj) {
        for solid in es.solids.iter().filter(|x| !x.triangles.is_empty()) {
            let mut points: HashMap<[i64; 3], usize> = HashMap::new();
            let mut faces = Vec::with_capacity(solid.triangles.len());
            for t in z_up(&es, solid) {
                let Some(n) = normal(&t) else {
                    continue;
                };
                let mut ids = [0; 3];
                for (k, v) in t.iter().enumerate() {
                    let key = v.map(|c| (c * 1000.0).round() as i64);
                    ids[k] = match points.get(&key) {
                        Some(&id) => id,
                        None => {
                            let id = s.add(&format!("CARTESIAN_POINT('',({},{},{}))", real(v[0]), real(v[1]), real(v[2])));
                            points.insert(key, id);
                            id
                        }
                    };
                }
                if ids[0] == ids[1] || ids[1] == ids[2] || ids[0] == ids[2] {
                    continue;
                }
                let loop_ = s.add(&format!("POLY_LOOP('',(#{},#{},#{}))", ids[0], ids[1], ids[2]));
                let bound = s.add(&format!("FACE_OUTER_BOUND('',#{loop_},.T.)"));
                let normal = s.add(&format!("DIRECTION('',({},{},{}))", real(n[0]), real(n[1]), real(n[2])));
                // A reference direction perpendicular to the normal.
                let r = norm(sub(t[1], t[0]));
                let refd = s.add(&format!("DIRECTION('',({},{},{}))", real(r[0]), real(r[1]), real(r[2])));
                let ax = s.add(&format!("AXIS2_PLACEMENT_3D('',#{},#{normal},#{refd})", ids[0]));
                let plane = s.add(&format!("PLANE('',#{ax})"));
                faces.push(s.add(&format!("FACE_SURFACE('',(#{bound}),#{plane},.T.)")));
            }
            if faces.is_empty() {
                continue;
            }
            let list = faces.iter().map(|f| format!("#{f}")).collect::<Vec<_>>().join(",");
            let shell = s.add(&format!("CLOSED_SHELL('',({list}))"));
            let brep = s.add(&format!("FACETED_BREP('{}',#{shell})", step_text(&solid.name)));
            items.push(brep);
            let style = match styles.get(&solid.color) {
                Some(&st) => st,
                None => {
                    let [r, g, b] = solid.color.map(|c| real(c as f32 / 255.0));
                    let rgb = s.add(&format!("COLOUR_RGB('',{r},{g},{b})"));
                    let fill_colour = s.add(&format!("FILL_AREA_STYLE_COLOUR('',#{rgb})"));
                    let fill = s.add(&format!("FILL_AREA_STYLE('',(#{fill_colour}))"));
                    let area = s.add(&format!("SURFACE_STYLE_FILL_AREA(#{fill})"));
                    let side = s.add(&format!("SURFACE_SIDE_STYLE('',(#{area}))"));
                    let usage = s.add(&format!("SURFACE_STYLE_USAGE(.BOTH.,#{side})"));
                    let st = s.add(&format!("PRESENTATION_STYLE_ASSIGNMENT((#{usage}))"));
                    styles.insert(solid.color, st);
                    st
                }
            };
            styled.push(s.add(&format!("STYLED_ITEM('color',(#{style}),#{brep})")));
        }
    }
    let list = items.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",");
    let rep = s.add(&format!("FACETED_BREP_SHAPE_REPRESENTATION('',({list}),#{gctx})"));
    s.add(&format!("SHAPE_DEFINITION_REPRESENTATION(#{pds},#{rep})"));
    if !styled.is_empty() {
        let list = styled.iter().map(|i| format!("#{i}")).collect::<Vec<_>>().join(",");
        s.add(&format!("MECHANICAL_DESIGN_GEOMETRIC_PRESENTATION_REPRESENTATION('',({list}),#{gctx})"));
    }
    let mut out = String::from("ISO-10303-21;\nHEADER;\n");
    let _ = writeln!(out, "FILE_DESCRIPTION(('OpenECraft panel layout'),'2;1');");
    let _ = writeln!(out, "FILE_NAME('{}','{}',(''),(''),'OpenECraft','OpenECraft','');", step_text(file_name), step_text(&prj.info.date));
    out.push_str("FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n");
    out.push_str(&s.out);
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out
}

#[cfg(test)]
mod tests;
