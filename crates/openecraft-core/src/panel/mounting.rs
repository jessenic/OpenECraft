//! Explicit physical mounting relationships in a common enclosure frame.
use super::{Enclosure,ItemKind,PanelItem};
use std::collections::{BTreeMap,BTreeSet,VecDeque};

fn parents(items:&[PanelItem])->BTreeMap<u64,u64> {
    items.iter().filter_map(|item| {
        if item.mounted_on!=0 {return Some((item.id,item.mounted_on));}
        if item.mounting_explicit {return None;}
        let rails:Vec<_>=items.iter().filter(|rail|item.seated_on(rail)).collect();
        (rails.len()==1).then(||(item.id,rails[0].id))
    }).collect()
}

pub fn can_mount(e:&Enclosure,id:u64,parent:u64)->bool {
    if e.item(id).is_none() {return false;}
    if parent==0 {return true;}
    let (Some(item),Some(support))=(e.item(id),e.item(parent)) else {return false;};
    if item.face!=support.face || support.kind==ItemKind::Cutout {return false;}
    let mut seen=BTreeSet::new();let mut at=parent;
    while at!=0 {
        if at==id || !seen.insert(at) {return false;}
        at=e.item(at).map(|i|i.mounted_on).unwrap_or(0);
    }
    true
}

/// Include mounted descendants and legacy devices geometrically seated on rails.
pub fn mounted_items(e:&Enclosure,ids:&[u64])->Vec<u64> {
    let parents=parents(&e.items);let mut selected:BTreeSet<_>=ids.iter().copied().collect();
    loop {
        let add:Vec<_>=parents.iter().filter(|(id,parent)|!selected.contains(id) && selected.contains(parent)).map(|(id,_)|*id).collect();
        if add.is_empty() {break;}
        selected.extend(add);
    }
    selected.into_iter().collect()
}

/// Delete only the requested items and detach their surviving children in place.
pub fn remove_items(e:&mut Enclosure,ids:&[u64])->usize {
    let old=e.items.len();e.items.retain(|i|!ids.contains(&i.id));
    for item in &mut e.items {
        if ids.contains(&item.mounted_on) {item.mounted_on=0;item.mounting_explicit=true;item.seated_on_rail=false;}
    }
    old-e.items.len()
}

/// Carry descendants after moving their parents; explicitly moved children keep
/// their requested positions and carry their own descendants exactly once.
pub fn carry_mounts(e:&mut Enclosure,before:&[PanelItem],explicit:&[u64]) {
    let parents=parents(before);let old:BTreeMap<_,_>=before.iter().map(|i|(i.id,[i.x,i.y,i.z+i.d])).collect();
    let explicit:BTreeSet<_>=explicit.iter().copied().collect();
    let mut queue:VecDeque<_>=explicit.iter().copied().collect();let mut seen=BTreeSet::new();
    while let Some(id)=queue.pop_front() {
        if !seen.insert(id) {continue;}
        let (Some(item),Some(old))=(e.item(id),old.get(&id)) else {continue;};
        let delta=[item.x-old[0],item.y-old[1],item.z+item.d-old[2]];
        for child in e.items.iter_mut().filter(|i|parents.get(&i.id)==Some(&id) && !explicit.contains(&i.id)) {
            child.x+=delta[0];child.y+=delta[1];child.z+=delta[2];queue.push_back(child.id);
        }
    }
    super::capture_anchors(e,&seen.into_iter().collect::<Vec<_>>());
}

#[cfg(test)]
mod tests;
