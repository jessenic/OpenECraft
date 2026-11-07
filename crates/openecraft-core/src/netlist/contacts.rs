//! Wire contacts for pages with explicit junctions and directional device ports.
use super::*;

pub(super) fn enabled(prj:&Project,page:&Page)->bool {
    page.values.get("page.explicit_wire_junctions").is_some_and(|v|v=="true") || page.symbols.iter().any(|s|prj.symbol(&s.sym).is_some_and(|d|!d.pin_directions.is_empty()))
}

pub(super) fn scan(prj:&Project,page:&Page,bridge_terminals:bool)->PageScan {
    let mut uf=Uf {parent:Vec::new()};
    let wire_nodes:Vec<_>=page.wires.iter().map(|_|uf.add()).collect();
    let explicit=page.values.get("page.explicit_wire_junctions").is_some_and(|v|v=="true");
    for (i,a) in page.wires.iter().enumerate() {
        for (j,b) in page.wires.iter().enumerate().skip(i+1) {
            let ends=[a.a,a.b].iter().any(|p|p.same(b.a)||p.same(b.b));
            let interior=!explicit && ([a.a,a.b].iter().any(|p|on_segment_interior(*p,b.a,b.b)) || [b.a,b.b].iter().any(|p|on_segment_interior(*p,a.a,a.b)));
            let junction=page.junctions.iter().any(|p|on_segment(p.pos,a.a,a.b) && on_segment(p.pos,b.a,b.b));
            if ends||interior||junction {uf.union(wire_nodes[i],wire_nodes[j]);}
        }
    }
    let mut pins=Vec::new();let mut wires=Vec::new();let mut scan=PageScan::default();let mut bridges=Vec::new();
    let mut contacts:Vec<(P,u8,u64,usize)>=Vec::new();
    for (si,s) in page.symbols.iter().enumerate() {
        let Some(def)=prj.symbol(&s.sym) else {continue;};
        if matches!(def.func,Func::Overview|Func::Graphic) {continue;}
        let mut nodes=Vec::new();
        for (i,pin) in def.pins.iter().enumerate() {
            let q=def.pin_world(i,s.pos,s.rot,s.mirror);
            let mask=def.pin_connection_directions(i,s.rot,s.mirror);
            let side_contact=!explicit || pin.internal || !def.func.has_tag();
            let n=uf.add();let mut attached=false;
            for (w,&wire) in page.wires.iter().zip(&wire_nodes) {
                if q.same(w.a) || q.same(w.b) || side_contact && on_segment_interior(q,w.a,w.b) && [w.a,w.b].iter().any(|other|accepts(mask,*other-q)) {
                    uf.union(n,wire);attached=true;
                }
            }
            for &(other,direction,symbol,node) in &contacts {
                if other.same(q) && (symbol==s.id || compatible(mask,direction)) {uf.union(n,node);attached=true;}
            }
            contacts.push((q,mask,s.id,n));nodes.push(n);
            scan.connection_points.push((s.id,i,n));
            if def.func!=Func::Junction && !pin.internal {pins.push((si,s.id,i,n,def.is_fluid(),!attached));}
        }
        if bridge_terminals || !def.func.is_feedthrough() {
            for &(a,b) in &def.bridges {bridges.push((nodes[a],nodes[b]));}
        }
        if let Some(&n)=nodes.first() {
            if def.func==Func::Terminal {scan.terminals.push(((s.tag.clone(),s.label.clone()),n));}
            if matches!(def.func,Func::Plug(_)) && !s.tag.is_empty() {scan.plug_pins.push(((s.tag.clone(),s.label.clone()),n));}
            match def.func {
                Func::Potential if !s.label.is_empty()=>scan.named.push((format!("pot:{}",s.label),n)),
                Func::Interruption if !s.label.is_empty()=>scan.named.push((format!("int:{}",s.interruption_key()),n)),
                _=>{}
            }
        }
    }
    let wired:std::collections::HashSet<_>=wire_nodes.iter().map(|n|uf.find(*n)).collect();
    for (_,_,_,n,_,open) in &mut pins {if wired.contains(&uf.find(*n)) {*open=false;}}
    for (a,b) in bridges {uf.union(a,b);}
    wires.extend(page.wires.iter().zip(wire_nodes).map(|(w,n)|(w.id,n)));
    let mut local=HashMap::new();
    let mut node=|uf:&mut Uf,n:usize| {let root=uf.find(n);let next=local.len();*local.entry(root).or_insert(next)};
    scan.pins=pins.into_iter().map(|(si,id,pin,n,fluid,open)|(si,id,pin,node(&mut uf,n),fluid,open)).collect();
    scan.wires=wires.into_iter().map(|(id,n)|(id,node(&mut uf,n))).collect();
    for (_,_,n) in &mut scan.connection_points {*n=node(&mut uf,*n);}
    for (_,n) in &mut scan.terminals {*n=node(&mut uf,*n);}
    for (_,n) in &mut scan.plug_pins {*n=node(&mut uf,*n);}
    for (_,n) in &mut scan.named {*n=node(&mut uf,*n);}
    scan.nodes=local.len();scan
}

fn accepts(mask:u8,delta:P)->bool {
    let bit=if delta.x.abs()>delta.y.abs() {if delta.x>0. {2}else{8}} else if delta.y>0. {4}else{1};
    mask & bit !=0
}

fn on_segment(q:P,a:P,b:P)->bool {q.same(a)||q.same(b)||on_segment_interior(q,a,b)}

fn compatible(a:u8,b:u8)->bool {
    let opposite=((a<<2)|(a>>2))&15;
    a==15 || b==15 || opposite&b!=0
}

struct PointCounts {
    pos:P,
    count:usize,
    wires:HashMap<u64,usize>,
}

fn point_counts(prj:&Project,page:&Page,q:P)->PointCounts {
    let mut uf=Uf {parent:Vec::new()};let mut wires=Vec::new();let mut sizes=Vec::new();
    let explicit=page.values.get("page.explicit_wire_junctions").is_some_and(|v|v=="true");
    let junction=page.junctions.iter().any(|j|j.pos.same(q));
    for w in &page.wires {
        if !on_segment(q,w.a,w.b) {continue;}
        let endpoint=q.same(w.a)||q.same(w.b);let n=uf.add();sizes.push((n,if endpoint {1}else{2}));
        for &(_,other,ends) in &wires {
            if junction || endpoint && ends || !explicit && (endpoint||ends) {uf.union(n,other);}
        }
        wires.push((w,n,endpoint));
    }
    let mut pins=Vec::new();
    for s in &page.symbols {
        let Some(def)=prj.symbol(&s.sym) else {continue;};
        if matches!(def.func,Func::Graphic|Func::Overview) {continue;}
        for i in 0..def.pins.len() {
            if !def.pin_world(i,s.pos,s.rot,s.mirror).same(q) {continue;}
            let n=uf.add();sizes.push((n,1));let mask=def.pin_connection_directions(i,s.rot,s.mirror);
            let side_contact=!explicit || def.pins[i].internal || !def.func.has_tag();
            for &(w,node,endpoint) in &wires {
                if endpoint || side_contact && (accepts(mask,w.a-q) || accepts(mask,w.b-q)) {uf.union(n,node);}
            }
            for &(symbol,direction,node) in &pins {
                if symbol==s.id || compatible(mask,direction) {uf.union(n,node);}
            }
            pins.push((s.id,mask,n));
        }
    }
    let mut groups=HashMap::new();
    for (node,size) in sizes {*groups.entry(uf.find(node)).or_insert(0)+=size;}
    PointCounts {pos:q,count:groups.values().copied().max().unwrap_or(0),wires:wires.into_iter().map(|(w,n,_)|(w.id,groups[&uf.find(n)])).collect()}
}

fn all_counts(prj:&Project,page:&Page)->HashMap<(i64,i64),PointCounts> {
    let mut points:HashMap<_,_>=page.wires.iter().flat_map(|w|[w.a,w.b]).map(|q|(q.key(),q)).collect();
    for j in &page.junctions {points.insert(j.pos.key(),j.pos);}
    points.into_iter().map(|(key,q)|(key,point_counts(prj,page,q))).collect()
}

pub(super) fn counts(prj:&Project,page:&Page)->HashMap<(i64,i64),(P,usize)> {
    all_counts(prj,page).into_iter().map(|(key,c)|(key,(c.pos,c.count))).collect()
}

pub(super) fn open_wire_ends(prj:&Project,page:&Page)->Vec<(u64,P)> {
    let counts=all_counts(prj,page);
    let lookup=&counts;
    page.wires.iter().flat_map(|w|[w.a,w.b].into_iter().filter_map(move |q| {
        (lookup[&q.key()].wires[&w.id]==1).then_some((w.id,q))
    })).collect()
}

#[cfg(test)]
mod tests;
