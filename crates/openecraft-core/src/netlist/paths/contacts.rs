//! Physical routes keep crossing conductors separate even inside one electrical net.
use super::*;

pub(super) struct Graph {
    pub uf: Uf,
    pub nodes: HashMap<Key, usize>,
    pub edges: Vec<Segment>,
    pub local_edges: Vec<Segment>,
}

struct WirePoint {
    point: P,
    node: usize,
    endpoint: bool,
    a: P,
    b: P,
}

impl Attachment {
    fn key(&self) -> Key { Key::Pin(self.page,self.symbol,self.pin) }
}

impl Graph {
    fn local(&mut self,a:usize,b:usize,length:u64,wire:Option<(usize,u64)>) {
        if a==b {return;}
        self.uf.union(a,b);
        self.edges.push((a,b,length,wire));
        self.local_edges.push((a,b,length,wire));
    }
}

pub(super) fn build(prj:&Project,net:&Net,sites:&[Site],attachments:&[Attachment])->Graph {
    let mut graph=Graph {uf:Uf {parent:Vec::new()},nodes:HashMap::new(),edges:Vec::new(),local_edges:Vec::new()};
    for a in attachments {
        let n=node(&mut graph.nodes,&mut graph.uf,a.key());
        if let Some(name)=&a.name {
            let m=node(&mut graph.nodes,&mut graph.uf,Key::Named(name.clone()));
            graph.uf.union(n,m);graph.edges.push((n,m,0,None));
        }
        for &pin in &a.bridges {
            let m=node(&mut graph.nodes,&mut graph.uf,Key::Pin(a.page,a.symbol,pin));
            graph.local(n,m,0,None);
        }
    }
    let mut points:HashMap<(usize,(i64,i64)),Vec<WirePoint>>=HashMap::new();
    for &(page,wid) in &net.wires {
        let pg=&prj.pages[page];
        let Some(w)=pg.wires.iter().find(|w|w.id==wid) else {continue;};
        let mut cuts=vec![w.a,w.b];
        cuts.extend(pg.wires.iter().flat_map(|w|[w.a,w.b]).filter(|q|on_segment_interior(*q,w.a,w.b)));
        cuts.extend(attachments.iter().filter(|a|a.page==page && on_segment_interior(a.point,w.a,w.b)).map(|a|a.point));
        cuts.extend(pg.junctions.iter().map(|j|j.pos).filter(|q|on_segment_interior(*q,w.a,w.b)));
        cuts.sort_by(|a,b| ((a.x-w.a.x)*(w.b.x-w.a.x)+(a.y-w.a.y)*(w.b.y-w.a.y)).total_cmp(&((b.x-w.a.x)*(w.b.x-w.a.x)+(b.y-w.a.y)*(w.b.y-w.a.y))));
        cuts.dedup_by(|a,b|a.same(*b));
        for pair in cuts.windows(2) {
            let a=node(&mut graph.nodes,&mut graph.uf,wire_key(sites,page,wid,pair[0],pair[1]));
            let b=node(&mut graph.nodes,&mut graph.uf,wire_key(sites,page,wid,pair[1],pair[0]));
            graph.local(a,b,(((pair[1]-pair[0]).len()*1000.) as u64).max(1),Some((page,wid)));
            for (q,n) in [(pair[0],a),(pair[1],b)] {
                points.entry((page,q.key())).or_default().push(WirePoint {point:q,node:n,endpoint:q.same(w.a)||q.same(w.b),a:w.a,b:w.b});
            }
        }
    }
    join_contacts(prj,sites,attachments,&points,&mut graph);
    graph
}

fn join_contacts(prj:&Project,sites:&[Site],attachments:&[Attachment],points:&HashMap<(usize,(i64,i64)),Vec<WirePoint>>,graph:&mut Graph) {
    for (&(page,_),at) in points {
        let pg=&prj.pages[page];let q=at[0].point;
        let explicit=pg.values.get("page.explicit_wire_junctions").is_some_and(|v|v=="true");
        let tee=sites.iter().any(|s|s.page==page && !s.arms.is_empty() && s.points[0].same(q));
        let junction=pg.junctions.iter().any(|j|j.pos.same(q));
        if !tee {
            for (i,a) in at.iter().enumerate() {
                for b in &at[i+1..] {
                    if junction || a.endpoint && b.endpoint || !explicit && (a.endpoint||b.endpoint) {graph.local(a.node,b.node,0,None);}
                }
            }
        }
        for pin in attachments.iter().filter(|a|a.page==page && a.point.same(q)) {
            let n=graph.nodes[&pin.key()];
            for w in at {
                if w.endpoint || pin.side_contact && (super::super::contacts::accepts(pin.mask,w.a-q)||super::super::contacts::accepts(pin.mask,w.b-q)) {
                    graph.local(n,w.node,0,None);
                }
            }
        }
    }
    for (i,a) in attachments.iter().enumerate() {
        for b in &attachments[i+1..] {
            if a.page==b.page && a.point.same(b.point) && (a.symbol==b.symbol || super::super::contacts::compatible(a.mask,b.mask)) {
                graph.local(graph.nodes[&a.key()],graph.nodes[&b.key()],0,None);
            }
        }
    }
}

#[cfg(test)]
mod tests;
