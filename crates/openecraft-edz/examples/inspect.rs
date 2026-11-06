//! Offline inspection of an EMA file, EDZ archive, or vendor folder. Pass a part
//! number after a folder to inspect a single article and all its representations.
use openecraft_core::symlib::ema::Variant;
use openecraft_edz::folder::{decode_file, index};
use serde_json::json;
use std::{collections::BTreeMap, env, path::Path};

/// Stream one record per representation; the full vendor folder need not fit in memory.
fn reference_row(source: &Path, part: Option<&str>, v: &Variant) {
    let bridges: Vec<_> = v
        .definitions()
        .flat_map(|s| {
            s.bridges.iter().filter_map(move |&(a, b)| {
                Some(json!({
                    "definition":s.key,"from_slot":a+1,"from":s.pins.get(a)?.label,"to_slot":b+1,"to":s.pins.get(b)?.label
                }))
            })
        })
        .collect();
    let pins: Vec<_> =
        v.definitions().flat_map(|s| s.pins.iter().map(move |p| json!({"definition":s.key,"label":p.label,"local_position":p.pos}))).collect();
    println!(
        "{}",
        json!({"source":source,"part":part,"symbol":v.symbol.name,"function_type":v.function_type,"variant_id":v.variant_id,
        "pins":pins,"bridges":bridges,"unresolved":v.unresolved,"connection_markers":v.connection_markers,"transfers":v.transfers,"placeholders":v.placeholders,"warnings":v.warnings})
    );
}

fn audit(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        let root = std::path::absolute(path).map_err(|e| e.to_string())?;
        for file in index(&root)?.macros {
            let relative = file.strip_prefix(&root).map_err(|e| e.to_string())?;
            match decode_file(&file) {
                Ok(vs) => {
                    for v in vs {
                        reference_row(relative, None, &v);
                    }
                }
                Err(e) => println!("{}", json!({"source":relative,"error":e})),
            }
        }
    } else if path.extension().is_some_and(|s| s.eq_ignore_ascii_case("edz")) {
        for p in openecraft_edz::read(&std::fs::read(path).map_err(|e| e.to_string())?)?.parts {
            for item in p.eplan_variants {
                match serde_json::from_value::<Variant>(item.as_ref().clone()) {
                    Ok(v) => reference_row(path, Some(&p.number), &v),
                    Err(_) => println!("{}", json!({"source":path,"part":p.number,"diagnostic":item})),
                }
            }
        }
    } else {
        for v in decode_file(path)? {
            reference_row(path, None, &v);
        }
    }
    Ok(())
}

fn main() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("Expected an EMA, EDZ or folder path")?;
    let path = Path::new(&path);
    let selection = args.next();
    if selection.as_deref() == Some("--references") {
        return audit(path);
    }
    if path.is_dir() {
        let catalog = index(path)?;
        if let Some(number) = selection {
            println!("{}", serde_json::to_string(&catalog.part(&number)?.parts).map_err(|e| e.to_string())?);
        } else {
            let mut errors = BTreeMap::new();
            let mut issues = BTreeMap::<String, usize>::new();
            let (mut variants, mut drawable, mut contacts) = (0, 0, 0);
            for file in &catalog.macros {
                match decode_file(file) {
                    Ok(list) => {
                        for v in list {
                            variants += 1;
                            drawable += usize::from(v.definitions().any(|s| !s.prims.is_empty()));
                            contacts += v.definitions().map(|s| s.pins.len()).sum::<usize>();
                            for reason in v.warnings {
                                *issues.entry(reason).or_default() += 1;
                            }
                        }
                    }
                    Err(e) => {
                        errors.insert(file.display().to_string(), e);
                    }
                }
            }
            println!(
                "{}",
                json!({"files":catalog.macros.len(),"variants":variants,"drawable":drawable,"contacts":contacts,"errors":errors,"issues":issues})
            );
        }
    } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("edz")) {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        println!("{}", serde_json::to_string(&openecraft_edz::read(&bytes)?.parts).map_err(|e| e.to_string())?);
    } else {
        let variants = decode_file(path)?;
        if let Some(selection) = selection {
            let v = variants.iter().find(|v| format!("{}/{}", v.function_type, v.variant_id) == selection).ok_or("Unknown variant")?;
            let mut project = openecraft_core::model::Project::default();
            project.symbols.push(v.symbol.clone());
            let def = project.symbol(&v.symbol.key).ok_or("Empty symbol")?;
            let size = def.bounds;
            let pos = openecraft_core::geom::p(5., 5.) - size.min;
            let symbol = openecraft_core::model::SymbolInst { pos, sym: v.symbol.key.clone(), tag: "-BC".into(), ..Default::default() };
            println!(
                "{}",
                openecraft_core::svg::drawing_svg(size.width() + 10., size.height() + 10., |d| {
                    openecraft_core::render::draw_symbol(d, &def, &symbol, openecraft_core::render::Ink::Line);
                })
            );
        } else {
            println!("{}", serde_json::to_string(&variants).map_err(|e| e.to_string())?);
        }
    }
    Ok(())
}
