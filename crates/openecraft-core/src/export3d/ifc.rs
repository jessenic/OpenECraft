//! IFC4 export: project, site, building and storey with an electric distribution
//! board per enclosure that aggregates a building element proxy per solid
//! (triangulated face set with a surface style); millimetres, z up.

use super::*;

const GUID_CHARS: &[u8; 64] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";

/// IFC GlobalId: 128 bits in 22 base-64 characters.
pub(super) fn guid(value: u128) -> String {
    let mut s = String::with_capacity(22);
    s.push(GUID_CHARS[(value >> 126) as usize] as char);
    for k in (0..21).rev() {
        s.push(GUID_CHARS[((value >> (k * 6)) & 63) as usize] as char);
    }
    s
}

/// IFC string literal: `'` doubled, `\` escaped, non-ASCII as `\X2\…\X0\`.
pub(super) fn text(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                out.push_str("\\X2\\");
                for unit in c.encode_utf16(&mut [0; 2]) {
                    let _ = write!(out, "{unit:04X}");
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    out
}

struct Ifc {
    out: String,
    next: usize,
    /// Seed of the GlobalIds, so that the same project gives the same ids.
    seed: u64,
}

impl Ifc {
    fn add(&mut self, entity: &str) -> usize {
        let id = self.next;
        self.next += 1;
        let _ = writeln!(self.out, "#{id}={entity};");
        id
    }

    /// GlobalId of the entity that is added next.
    fn guid(&self) -> String {
        // splitmix64 of seed and entity number, twice for 128 bits.
        let mix = |mut z: u64| {
            z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            z ^ (z >> 31)
        };
        let a = mix(self.seed ^ (self.next as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let b = mix(a ^ 0x5851_f42d_4c95_7f2d);
        guid(((a as u128) << 64) | b as u128)
    }
}

/// An angle in degrees as an IFC compound plane angle: degrees, minutes, seconds and
/// millionths of a second, all with its sign.
fn compound_angle(v: f64) -> String {
    let micro = (v.abs() * 3600.0 * 1e6).round() as i64;
    let sign = if v < 0.0 { -1 } else { 1 };
    let (d, m, sec, mic) = (micro / 3_600_000_000, micro / 60_000_000 % 60, micro / 1_000_000 % 60, micro % 1_000_000);
    format!("({},{},{},{})", sign * d, sign * m, sign * sec, sign * mic)
}

/// The scene as IFC4 (STEP physical file).
pub fn ifc(prj: &Project, file_name: &str) -> String {
    let name = if prj.info.name.is_empty() { "Panel layout" } else { &prj.info.name };
    let seed = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let mut f = Ifc { out: String::new(), next: 1, seed };
    let origin = f.add("IFCCARTESIANPOINT((0.,0.,0.))");
    let world = f.add(&format!("IFCAXIS2PLACEMENT3D(#{origin},$,$)"));
    let ctx = f.add(&format!("IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-05,#{world},$)"));
    let body = f.add(&format!("IFCGEOMETRICREPRESENTATIONSUBCONTEXT('Body','Model',*,*,*,*,#{ctx},$,.MODEL_VIEW.,$)"));
    let length = f.add("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)");
    let angle = f.add("IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.)");
    let units = f.add(&format!("IFCUNITASSIGNMENT((#{length},#{angle}))"));
    let project = f.add(&format!("IFCPROJECT('{}',$,{},$,$,$,$,(#{ctx}),#{units})", f.guid(), text(name)));
    let mut placement = f.add(&format!("IFCLOCALPLACEMENT($,#{world})"));
    // The plot: its name, where it lies (WGS 84) and its height above sea level.
    let plot = &prj.layout.plot;
    let site_name = if plot.name.is_empty() { text("Site") } else { text(&plot.name) };
    let r = plot.reference.as_ref();
    let deg = |v: Option<f64>| v.map_or_else(|| "$".to_string(), compound_angle);
    let (lat, lon) = (deg(r.and_then(|r| r.latitude)), deg(r.and_then(|r| r.longitude)));
    let elevation = r.map_or_else(|| "$".to_string(), |r| format!("{:.1}", (r.height * 1000.0 - f64::from(r.y))));
    let site = f.add(&format!("IFCSITE('{}',$,{site_name},$,$,#{placement},$,$,.ELEMENT.,{lat},{lon},{elevation},$,$)", f.guid()));
    placement = f.add(&format!("IFCLOCALPLACEMENT(#{placement},#{world})"));
    let building = f.add(&format!("IFCBUILDING('{}',$,'Building',$,$,#{placement},$,$,.ELEMENT.,$,$,$)", f.guid()));
    placement = f.add(&format!("IFCLOCALPLACEMENT(#{placement},#{world})"));
    let storey = f.add(&format!("IFCBUILDINGSTOREY('{}',$,'Storey',$,$,#{placement},$,$,.ELEMENT.,0.)", f.guid()));
    for (whole, part) in [(project, site), (site, building), (building, storey)] {
        f.add(&format!("IFCRELAGGREGATES('{}',$,$,$,#{whole},(#{part}))", f.guid()));
    }
    let mut styles: HashMap<([u8; 3], bool), usize> = HashMap::new();
    let mut boards = Vec::new();
    for es in scene(prj) {
        let board_placement = f.add(&format!("IFCLOCALPLACEMENT(#{placement},#{world})"));
        let board = f.add(&format!("IFCELECTRICDISTRIBUTIONBOARD('{}',$,{},$,$,#{board_placement},$,$,.DISTRIBUTIONBOARD.)", f.guid(), text(&es.name)));
        boards.push(board);
        let mut parts = Vec::new();
        for solid in es.solids.iter().filter(|s| !s.triangles.is_empty()) {
            let mut index: HashMap<[i64; 3], usize> = HashMap::new();
            let mut points: Vec<String> = Vec::new();
            let mut faces: Vec<String> = Vec::new();
            for t in z_up(&es, solid).iter().filter(|t| normal(t).is_some()) {
                let ids = t.map(|v| {
                    *index.entry(v.map(|c| (c * 1000.0).round() as i64)).or_insert_with(|| {
                        points.push(format!("({},{},{})", real(v[0]), real(v[1]), real(v[2])));
                        points.len()
                    })
                });
                if ids[0] != ids[1] && ids[1] != ids[2] && ids[0] != ids[2] {
                    faces.push(format!("({},{},{})", ids[0], ids[1], ids[2]));
                }
            }
            if faces.is_empty() {
                continue;
            }
            let list = f.add(&format!("IFCCARTESIANPOINTLIST3D(({}))", points.join(",")));
            let set = f.add(&format!("IFCTRIANGULATEDFACESET(#{list},$,$,({}),$)", faces.join(",")));
            let style = match styles.get(&(solid.color, solid.transparent)) {
                Some(&s) => s,
                None => {
                    let [r, g, b] = solid.color.map(|c| real(c as f32 / 255.0));
                    let rgb = f.add(&format!("IFCCOLOURRGB($,{r},{g},{b})"));
                    let transparency = if solid.transparent { "0.75" } else { "0." };
                    let rendering = f.add(&format!("IFCSURFACESTYLERENDERING(#{rgb},{transparency},$,$,$,$,$,$,.NOTDEFINED.)"));
                    let s = f.add(&format!("IFCSURFACESTYLE($,.BOTH.,(#{rendering}))"));
                    styles.insert((solid.color, solid.transparent), s);
                    s
                }
            };
            f.add(&format!("IFCSTYLEDITEM(#{set},(#{style}),$)"));
            let rep = f.add(&format!("IFCSHAPEREPRESENTATION(#{body},'Body','Tessellation',(#{set}))"));
            let shape = f.add(&format!("IFCPRODUCTDEFINITIONSHAPE($,$,(#{rep}))"));
            let part_placement = f.add(&format!("IFCLOCALPLACEMENT(#{board_placement},#{world})"));
            parts.push(f.add(&format!(
                "IFCBUILDINGELEMENTPROXY('{}',$,{},$,$,#{part_placement},#{shape},{},.NOTDEFINED.)",
                f.guid(),
                text(&solid.name),
                text(&solid.name)
            )));
        }
        if !parts.is_empty() {
            let list = parts.iter().map(|p| format!("#{p}")).collect::<Vec<_>>().join(",");
            f.add(&format!("IFCRELAGGREGATES('{}',$,$,$,#{board},({list}))", f.guid()));
        }
    }
    if !boards.is_empty() {
        let list = boards.iter().map(|p| format!("#{p}")).collect::<Vec<_>>().join(",");
        f.add(&format!("IFCRELCONTAINEDINSPATIALSTRUCTURE('{}',$,$,$,({list}),#{storey})", f.guid()));
    }
    let mut out = String::from("ISO-10303-21;\nHEADER;\n");
    out.push_str("FILE_DESCRIPTION(('ViewDefinition [ReferenceView_V1.2]'),'2;1');\n");
    let _ = writeln!(out, "FILE_NAME({},{},(''),(''),'OpenECraft','OpenECraft','');", text(file_name), text(&prj.info.date));
    out.push_str("FILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n");
    out.push_str(&f.out);
    out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    out
}
