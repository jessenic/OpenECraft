//! PXF placeholder control data. Placed objects already contain the selected
//! value set. Preserve alternate assignments separately from schematic pins.
use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Placeholder {
    pub object: String,
    pub name: String,
    pub selected: String,
    pub variables: Vec<String>,
    pub sets: Vec<Set>,
    pub bindings: Vec<Binding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Set {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Binding {
    pub object: String,
    pub property: String,
    pub index: String,
    pub expression: String,
}

fn list(raw: &str) -> Vec<String> {
    let mut fields = vec![String::new()];
    let mut iter = raw.chars().peekable();
    while let Some(c) = iter.next() {
        match c {
            ';' => fields.push(String::new()),
            '\\' if matches!(iter.peek(), Some(';' | '\\')) => fields.last_mut().unwrap().push(iter.next().unwrap()),
            _ => fields.last_mut().unwrap().push(c),
        }
    }
    fields
}

pub(super) fn read(el: &Element) -> Placeholder {
    Placeholder {
        object: el.attribute("A1").unwrap_or("").into(),
        name: el.attribute("A1892").unwrap_or("").into(),
        selected: el.child("P11").and_then(|p| p.attribute("P19301")).unwrap_or("").into(),
        variables: el.attribute("A1896").map(list).unwrap_or_default(),
        sets: el
            .all("S114x1894")
            .map(|s| Set { name: s.attribute("A2011").unwrap_or("").into(), values: s.attribute("A2012").map(list).unwrap_or_default() })
            .collect(),
        bindings: el
            .all("S109x1895")
            .flat_map(|s| {
                s.all("S115x1904").map(move |b| Binding {
                    object: s.attribute("R1906").unwrap_or("").into(),
                    property: b.attribute("A1911").unwrap_or("").into(),
                    index: b.attribute("A1912").unwrap_or("").into(),
                    expression: b.attribute("A1913").unwrap_or("").into(),
                })
            })
            .collect(),
    }
}
