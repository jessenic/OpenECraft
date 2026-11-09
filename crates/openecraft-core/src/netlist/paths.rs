//! Physical target paths through user-defined junctions with spaced connection points.

use super::*;
use crate::model::{Junction, TeeDir};
use std::collections::{HashMap, HashSet};
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub(super) struct TracedPaths {
    pub pairs: Vec<[PinRef;2]>,
    pub wire_routes: Vec<Vec<(usize,u64)>>,
}

#[derive(Clone)]
struct Site {
    page: usize,
    symbol: Option<u64>,
    points: Vec<P>,
    links: Vec<(usize, usize)>,
    arms: Vec<TeeDir>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Pin(usize, u64, usize),
    Wire(usize, u64, (i64, i64)),
    Port(usize, usize),
    Named(String),
}

impl Site {
    fn key(&self, site: usize, port: usize) -> Key {
        if self.arms.is_empty() { Key::Pin(self.page, self.symbol.expect("symbol junction"), port) } else { Key::Port(site, port) }
    }
}

pub(super) struct Paths {
    nets: HashMap<usize, Vec<Site>>,
    attachments: HashMap<usize,Vec<Attachment>>,
}

struct Attachment {
    page: usize,
    point: P,
    symbol: u64,
    pin: usize,
    mask: u8,
    side_contact: bool,
    name: Option<String>,
    bridges: Vec<usize>,
}

impl Paths {
    pub fn new(prj: &Project, open: &Netlist) -> Self {
        Self::with_local_routes(prj,open,false)
    }

    fn with_local_routes(prj:&Project,open:&Netlist,all:bool)->Self {
        let mut nets: HashMap<usize, Vec<Site>> = HashMap::new();
        let symbols=prj.symbol_table();
        for pg in &prj.pages {
            for w in &pg.wires {
                if all || pg.values.get("page.use_drawn_wire_targets").is_some_and(|v|v=="true") || w.explicit_junctions==Some(true) {
                    if let Some(&id)=open.wire_net.get(&w.id).filter(|id|!open.nets[**id].fluid) {nets.entry(id).or_default();}
                }
            }
        }
        for (page, pg) in prj.pages.iter().enumerate() {
            for s in &pg.symbols {
                let Some(def) = symbols.get(&s.sym).filter(|d| d.directed_junction || d.func.has_tag() && !d.func.is_feedthrough() && !d.bridges.is_empty()) else { continue };
                let site = Site { page, symbol: Some(s.id), points: (0..def.pins.len()).map(|i| def.pin_world(i, s.pos, s.rot, s.mirror)).collect(), links: def.bridges.clone(), arms: Vec::new() };
                let ids: HashSet<usize> = (0..def.pins.len()).filter_map(|i| open.connection_point_net.get(&(s.id, i)).copied()).collect();
                for id in ids {
                    if open.nets[id].fluid {continue;}
                    let sites = nets.entry(id).or_default();
                    if def.directed_junction { sites.push(site.clone()); }
                }
            }
        }
        let mut attachments:HashMap<usize,Vec<Attachment>>=HashMap::new();
        for (page,pg) in prj.pages.iter().enumerate() {
            for s in &pg.symbols {
                let Some(def)=symbols.get(&s.sym) else {continue;};
                let name=match def.func {
                    Func::Potential if !s.label.is_empty()=>Some(format!("pot:{}",s.label)),
                    Func::Interruption if !s.label.is_empty()=>Some(format!("int:{}",s.interruption_key())),
                    _=>None,
                };
                for pin in 0..def.pins.len() {
                    let Some(&id)=open.connection_point_net.get(&(s.id,pin)).filter(|id|nets.contains_key(id)) else {continue;};
                    let bridges=if !def.directed_junction && !def.func.has_tag() {
                        def.bridges.iter().filter(|(a,_)|*a==pin).map(|(_,b)|*b).collect()
                    } else {Vec::new()};
                    attachments.entry(id).or_default().push(Attachment {page,point:def.pin_world(pin,s.pos,s.rot,s.mirror),symbol:s.id,pin,mask:def.pin_connection_directions(pin,s.rot,s.mirror),side_contact:def.pins[pin].internal || !def.func.has_tag(),name:name.clone(),bridges});
                }
            }
        }
        Self { nets,attachments }
    }

    pub fn trace(&self, prj: &Project, open: &Netlist, net: &Net, native: &[(usize, &Junction)]) -> Option<TracedPaths> {
        let id = open.wire_net.get(&net.wires.first()?.1)?;
        let mut sites = self.nets.get(id)?.clone();
        for &(page, j) in native {
            let arms = j.arms();
            let links = j.links().into_iter().filter_map(|(a,b)| Some((arms.iter().position(|d| *d == a)?, arms.iter().position(|d| *d == b)?))).collect();
            sites.push(Site { page, symbol: None, points: vec![j.pos; arms.len()], links, arms });
        }
        Some(trace(prj, net, &sites,self.attachments.get(id).map(Vec::as_slice).unwrap_or_default()))
    }
}

pub(super) fn local_route(prj:&Project,open:&Netlist,from:(u64,usize),to:(u64,usize))->Option<Vec<(usize,u64)>> {
    let id=*open.connection_point_net.get(&from)?;
    if open.connection_point_net.get(&to)!=Some(&id) {return None;}
    let paths=Paths::with_local_routes(prj,open,true);
    let net=&open.nets[id];
    let mut sites=paths.nets.get(&id)?.clone();
    for (page,j) in junctions::Tees::new(prj).on_net(prj,net) {
        let arms=j.arms();
        let links=j.links().into_iter().filter_map(|(a,b)|Some((arms.iter().position(|d|*d==a)?,arms.iter().position(|d|*d==b)?))).collect();
        sites.push(Site {page,symbol:None,points:vec![j.pos;arms.len()],links,arms});
    }
    let graph=contacts::build(prj,net,&sites,paths.attachments.get(&id)?.as_slice());
    let pin_key=|(symbol,pin)| {
        let page=prj.pages.iter().position(|p|p.symbols.iter().any(|s|s.id==symbol))?;
        Some(Key::Pin(page,symbol,pin))
    };
    let a=*graph.nodes.get(&pin_key(from)?)?;
    let b=*graph.nodes.get(&pin_key(to)?)?;
    let mut edges=graph.local_edges;
    for (i,site) in sites.iter().enumerate() {
        for &(a,b) in &site.links {
            if let (Some(a),Some(b))=(graph.nodes.get(&site.key(i,a)),graph.nodes.get(&site.key(i,b))) {edges.push((*a,*b,0,None));}
        }
    }
    let graph=graph_of(graph.uf.parent.len(),&edges);
    if !reachable_nodes(&graph,&HashMap::new(),a).contains(&b) {return None;}
    Some(wire_route(&graph,a,b))
}

fn node(nodes: &mut HashMap<Key, usize>, uf: &mut Uf, key: Key) -> usize {
    *nodes.entry(key).or_insert_with(|| uf.add())
}

fn wire_key(sites: &[Site], page: usize, wire: u64, q: P, other: P) -> Key {
    for (i, site) in sites.iter().enumerate().filter(|(_,s)| s.page == page && !s.arms.is_empty()) {
        if !site.points[0].same(q) { continue; }
        let delta = other - q;
        let direction = if delta.x.abs() > delta.y.abs() { if delta.x > 0. { TeeDir::Right } else { TeeDir::Left } }
                        else if delta.y > 0. { TeeDir::Down } else { TeeDir::Up };
        if let Some(port) = site.arms.iter().position(|d| *d == direction) { return site.key(i, port); }
    }
    Key::Wire(page, wire, q.key())
}

fn trace(prj: &Project, net: &Net, sites: &[Site], attachments:&[Attachment]) -> TracedPaths {
    let contacts::Graph {mut uf,nodes,mut edges,local_edges}=contacts::build(prj,net,sites,attachments);
    let mut pins: HashMap<usize,Vec<PinRef>> = HashMap::new();
    let physical=chained_pins(prj,net);
    let mut pin_nodes:HashMap<usize,Vec<PinRef>>=HashMap::new();
    for &pin in &physical {
        let s = &prj.pages[pin.page].symbols[pin.sym];
        let key = Key::Pin(pin.page,s.id,pin.pin);
        if let Some(n) = nodes.get(&key) {
            pins.entry(uf.find(*n)).or_default().push(pin);
            pin_nodes.entry(*n).or_default().push(pin);
        }
    }
    let ContactGraph {graph:connections,pins:contact_pins,canonical}=contact_graph(uf.parent.len(),&edges,&pin_nodes);
    let mut components = HashMap::new();
    for (i,site) in sites.iter().enumerate() {
        for port in 0..site.points.len() {
            if let Some(n) = nodes.get(&site.key(i,port)) {
                components.insert((i,port),canonical[*n]);
            }
        }
    }
    let reached=components.keys().filter_map(|&(site,port)| {
        let start=*nodes.get(&sites[site].key(site,port))?;
        Some(((site,port),reachable_pins(&connections,&contact_pins,canonical[start])))
    }).collect();
    let through=components.keys().filter_map(|&(site,port)| {
        let start=*nodes.get(&sites[site].key(site,port))?;
        let visited=reachable_nodes(&connections,&contact_pins,canonical[start]);
        let mut others:Vec<_>=components.keys().filter(|&&(s,p)|nodes.get(&sites[s].key(s,p)).is_some_and(|n|visited.contains(&canonical[*n]))).copied().collect();
        others.sort_unstable();
        Some(((site,port),others))
    }).collect();
    let context = Context { sites,reached,through };
    for (i,site) in sites.iter().enumerate() {
        for &(a,b) in &site.links {
            if let (Some(a),Some(b))=(nodes.get(&site.key(i,a)),nodes.get(&site.key(i,b))) {
                edges.push((*a,*b,0,None));
            }
        }
    }
    let mut collected=Collected::new(&physical);
    for (i,site) in sites.iter().enumerate() {
        for &(a,b) in &site.links {
            for from in context.beyond(i,a) {
                for to in context.beyond(i,b) { collected.add(from,to,false); }
            }
        }
    }
    let mut groups: Vec<_> = pins.values().collect();
    groups.sort_by_key(|p| (p[0].page,p[0].sym,p[0].pin));
    let ContactGraph {graph:local_graph,pins:local_pins,..}=contact_graph(uf.parent.len(),&local_edges,&pin_nodes);
    for [a,b] in drawn_pairs(&local_graph,&local_pins) {collected.add(a,b,false);}
    for group in groups { for pair in group.windows(2) { collected.add(pair[0],pair[1],true); } }
    let pairs=collected.pairs;
    let graph=graph_of(uf.parent.len(),&edges);
    let key_of=|pin:PinRef| {
        let s=&prj.pages[pin.page].symbols[pin.sym];
        nodes.get(&Key::Pin(pin.page,s.id,pin.pin)).copied()
    };
    let wire_routes=pairs.iter().map(|pair| {
        match (key_of(pair[0]),key_of(pair[1])) {
            (Some(a),Some(b))=>wire_route(&graph,a,b),
            _=>Vec::new(),
        }
    }).collect();
    let mut traced=TracedPaths {pairs,wire_routes};
    core_routes::extend(prj,net,&edges,&pin_nodes,&nodes,&mut traced);
    traced
}

type Edge=(usize,u64,Option<(usize,u64)>);
type Segment=(usize,usize,u64,Option<(usize,u64)>);

fn graph_of(size:usize,edges:&[Segment])->Vec<Vec<Edge>> {
    let mut graph=vec![Vec::new();size];
    for &(a,b,length,wire) in edges {
        graph[a].push((b,length,wire));graph[b].push((a,length,wire));
    }
    graph
}

struct ContactGraph {
    graph: Vec<Vec<Edge>>,
    pins: HashMap<usize,Vec<PinRef>>,
    canonical: Vec<usize>,
}

fn contact_graph(size:usize,edges:&[Segment],pins:&HashMap<usize,Vec<PinRef>>)->ContactGraph {
    let mut uf=Uf {parent:(0..size).collect()};
    for &(a,b,length,wire) in edges {if length==0 && wire.is_none() {uf.union(a,b);}}
    let canonical:Vec<_>=(0..size).map(|n|uf.find(n)).collect();
    let segments:Vec<_>=edges.iter().filter_map(|&(a,b,length,wire)| {
        let (a,b)=(canonical[a],canonical[b]);(a!=b).then_some((a,b,length,wire))
    }).collect();
    let mut contacts:HashMap<usize,Vec<PinRef>>=HashMap::new();
    for (&at,pins) in pins {contacts.entry(canonical[at]).or_default().extend(pins);}
    ContactGraph {graph:graph_of(size,&segments),pins:contacts,canonical}
}

fn reachable_pins(graph:&[Vec<Edge>],pins:&HashMap<usize,Vec<PinRef>>,start:usize)->Vec<PinRef> {
    let mut out:Vec<_>=reachable_nodes(graph,pins,start).iter().filter_map(|at|pins.get(at)).flatten().copied().collect();
    out.sort_by_key(|p: &PinRef|(p.page,p.sym,p.pin));out.dedup();out
}

fn reachable_nodes(graph:&[Vec<Edge>],pins:&HashMap<usize,Vec<PinRef>>,start:usize)->HashSet<usize> {
    let mut stack=vec![start];let mut visited=HashSet::new();
    while let Some(at)=stack.pop() {
        if !visited.insert(at) {continue;}
        if pins.contains_key(&at) {continue;}
        stack.extend(graph[at].iter().map(|e|e.0));
    }
    visited
}

fn drawn_pairs(graph:&[Vec<Edge>],pins:&HashMap<usize,Vec<PinRef>>)->Vec<[PinRef;2]> {
    let mut out=Vec::new();
    for (&start,physical) in pins {
        for &pin in physical {
            let mut stack=vec![start];let mut visited=HashSet::new();
            while let Some(at)=stack.pop() {
                if !visited.insert(at) {continue;}
                if let Some(others)=pins.get(&at) {
                    for &other in others {if other!=pin {out.push([pin,other]);}}
                    if at!=start {continue;}
                }
                let exits:HashSet<_>=graph[at].iter().map(|e|e.0).filter(|n|*n!=at).collect();
                if at!=start && exits.len()>2 {continue;}
                stack.extend(exits);
            }
        }
    }
    for pair in &mut out {
        if (pair[0].page,pair[0].sym,pair[0].pin)>(pair[1].page,pair[1].sym,pair[1].pin) {pair.swap(0,1);}
    }
    out.sort_by_key(|p|(p[0].page,p[0].sym,p[0].pin,p[1].page,p[1].sym,p[1].pin));
    out.dedup();
    out
}

struct Collected {
    pairs:Vec<[PinRef;2]>,
    seen:HashSet<(PinRef,PinRef)>,
    pins:HashMap<PinRef,usize>,
    uf:Uf,
}

impl Collected {
    fn new(pins:&[PinRef])->Self {
        Self {pairs:Vec::new(),seen:HashSet::new(),pins:pins.iter().enumerate().map(|(i,p)|(*p,i)).collect(),uf:Uf {parent:(0..pins.len()).collect()}}
    }
    fn add(&mut self,a:PinRef,b:PinRef,only_separate:bool) {
        let (Some(&ia),Some(&ib))=(self.pins.get(&a),self.pins.get(&b)) else {return;};
        if a==b || only_separate && self.uf.find(ia)==self.uf.find(ib) {return;}
        let key=if (a.page,a.sym,a.pin)<=(b.page,b.sym,b.pin) {(a,b)} else {(b,a)};
        if self.seen.insert(key) {self.pairs.push([a,b]);self.uf.union(ia,ib);}
    }
}

fn wire_route(graph:&[Vec<Edge>],start:usize,end:usize)->Vec<(usize,u64)> {
    let mut distances=vec![u64::MAX;graph.len()];
    let mut previous=vec![None;graph.len()];
    let mut queue=BinaryHeap::from([(Reverse(0),start)]);
    distances[start]=0;
    while let Some((Reverse(cost),at))=queue.pop() {
        if at==end {break;}
        if cost!=distances[at] {continue;}
        for &(next,length,wire) in &graph[at] {
            if cost+length<distances[next] {
                distances[next]=cost+length;
                previous[next]=Some((at,wire));
                queue.push((Reverse(cost+length),next));
            }
        }
    }
    let mut out=Vec::new();
    let mut at=end;
    while let Some((from,wire))=previous[at] {
        if let Some(wire)=wire {if !out.contains(&wire) {out.push(wire);}}
        at=from;
    }
    out.reverse();
    out
}

struct Context<'a> {
    sites: &'a [Site],
    reached:HashMap<(usize,usize),Vec<PinRef>>,
    through:HashMap<(usize,usize),Vec<(usize,usize)>>,
}

impl Context<'_> {
    fn beyond(&self,site: usize,port: usize) -> Vec<PinRef> {
        let mut out = Vec::new();
        let mut stack=vec![(site,port)];
        let mut visited=HashSet::new();
        while let Some(at)=stack.pop() {
            if !visited.insert(at) {continue;}
                for &pin in self.reached.get(&at).into_iter().flatten() {
                    if !out.contains(&pin) {out.push(pin);}
                }
            for &(next,entry) in self.through.get(&at).into_iter().flatten() {
                if next==at.0 {continue;}
                for &(a,b) in &self.sites[next].links {
                    if a==entry {stack.push((next,b));}
                    else if b==entry {stack.push((next,a));}
                }
            }
        }
        out
    }
}

mod contacts;

mod core_routes;

#[cfg(test)]
mod tests;
