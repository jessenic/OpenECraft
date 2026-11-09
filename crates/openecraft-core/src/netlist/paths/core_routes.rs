use std::cmp::Reverse;
use std::collections::{BinaryHeap,HashSet};
use super::{contact_graph,Key,PinRef,Project,Net,Segment,TracedPaths};
use std::collections::{HashMap,BTreeSet};

#[derive(Clone)]
struct Arc {to:usize,back:usize,available:bool,cost:i64,original:bool,wire:Option<(usize,u64)>}

fn arc(graph:&mut [Vec<Arc>],a:usize,b:usize,cost:i64,wire:Option<(usize,u64)>) {
    let (forward,back)=(graph[b].len(),graph[a].len());
    graph[a].push(Arc {to:b,back:forward,available:true,cost,original:true,wire});
    graph[b].push(Arc {to:a,back,available:false,cost:-cost,original:false,wire:None});
}

fn augment(graph:&mut [Vec<Arc>],start:usize,end:usize,potential:&mut [i64])->bool {
    let mut distances=vec![i64::MAX;graph.len()];let mut previous=vec![None;graph.len()];
    let mut queue=BinaryHeap::from([(Reverse(0),start)]);distances[start]=0;
    while let Some((Reverse(cost),at))=queue.pop() {
        if cost!=distances[at] {continue}
        for (index,edge) in graph[at].iter().enumerate().filter(|(_,e)|e.available) {
            let distance=cost+edge.cost+potential[at]-potential[edge.to];
            if distance<distances[edge.to] {
                distances[edge.to]=distance;previous[edge.to]=Some((at,index));queue.push((Reverse(distance),edge.to));
            }
        }
    }
    if previous[end].is_none() {return false}
    for (p,d) in potential.iter_mut().zip(distances) {if d!=i64::MAX {*p+=d;}}
    let mut at=end;
    while let Some((from,index))=previous[at] {
        let back=graph[from][index].back;graph[from][index].available=false;graph[at][back].available=true;at=from;
    }
    true
}

// A simple path containing a specified edge equals two vertex-disjoint paths
// from its endpoints to the two terminals. Residual flow can reroute the first
// path so a greedy choice cannot trap the second one behind the same junction.
fn through(size:usize,segments:&[Segment],a:usize,b:usize,witness:usize,blocked:&HashSet<usize>)->Option<(u64,Vec<(usize,u64)>)> {
    let &(u,v,length,wire)=segments.get(witness)?;
    if a==b || u==v {return None}
    let (source,sink)=(size*2,size*2+1);let mut graph=vec![Vec::new();size*2+2];
    for at in 0..size {if !blocked.contains(&at) || at==a || at==b {arc(&mut graph,at*2,at*2+1,0,None);}}
    for (index,&(from,to,cost,wire)) in segments.iter().enumerate() {
        if index==witness {continue}
        arc(&mut graph,from*2+1,to*2,cost as i64,wire);arc(&mut graph,to*2+1,from*2,cost as i64,wire);
    }
    for terminal in [a,b] {arc(&mut graph,source,terminal*2,0,None);}
    for terminal in [u,v] {arc(&mut graph,terminal*2+1,sink,0,None);}
    let mut potential=vec![0;graph.len()];
    if !augment(&mut graph,source,sink,&mut potential) || !augment(&mut graph,source,sink,&mut potential) {return None}
    let mut halves=Vec::new();let mut total=length;
    for terminal in [a,b] {
        let mut at=terminal*2;let mut seen=HashSet::new();let mut wires=Vec::new();
        while at!=sink {
            if !seen.insert(at) {return None}
            let edge=graph[at].iter().find(|e|e.original && !e.available)?;
            total+=edge.cost as u64;if let Some(wire)=edge.wire {wires.push(wire);}at=edge.to;
        }
        halves.push(wires);
    }
    let mut result=halves.remove(0);result.extend(wire);result.extend(halves.remove(0).into_iter().rev());
    let mut seen=HashSet::new();result.retain(|wire|seen.insert(*wire));Some((total,result))
}


fn reached(graph:&[Vec<(usize,usize)>],physical:&HashSet<usize>,start:usize,removed:usize)->HashSet<usize> {
    let mut stack=vec![start];let mut visited=HashSet::new();let mut out=HashSet::new();
    while let Some(at)=stack.pop() {
        if !visited.insert(at) {continue}
        if physical.contains(&at) {out.insert(at);continue}
        stack.extend(graph[at].iter().filter(|(_,edge)|*edge!=removed).map(|(next,_)|*next));
    }
    out
}

pub(super) fn extend(prj:&Project,net:&Net,segments:&[Segment],pins:&HashMap<usize,Vec<PinRef>>,
                     nodes:&HashMap<Key,usize>,paths:&mut TracedPaths) {
    let labels:HashMap<_,_>=net.wires.iter().filter_map(|&(page,id)| {
        let wire=prj.pages[page].wires.iter().find(|w|w.id==id)?;
        (!wire.cable.is_empty()).then(||((page,id),(wire.cable.clone(),wire.core.clone())))
    }).collect();
    let groups:BTreeSet<_>=labels.values().filter(|(_,core)|!core.is_empty()).cloned().collect();
    if groups.is_empty() {return}
    let size=nodes.values().max().map_or(0,|n|n+1);
    let contacts=contact_graph(size,segments,pins);
    let blocked:HashSet<_>=contacts.pins.keys().copied().collect();
    let key_of=|pin:PinRef| {
        let symbol=&prj.pages[pin.page].symbols[pin.sym];
        nodes.get(&Key::Pin(pin.page,symbol.id,pin.pin)).map(|n|contacts.canonical[*n])
    };
    let pairs:Vec<_>=paths.pairs.iter().enumerate().filter_map(|(index,pair)| {
        let (a,b)=(key_of(pair[0])?,key_of(pair[1])?);
        let represented=paths.wire_routes[index].iter().filter_map(|wire|labels.get(wire).map(|label|(wire.1,label)))
            .min_by_key(|(id,_)|*id).map(|(_,label)|label.clone());
        (a!=b).then_some((*pair,a,b,represented))
    }).collect();
    for group in &groups {
        let allowed:Vec<_>=segments.iter().filter_map(|&(a,b,length,wire)| {
            let (a,b)=(contacts.canonical[a],contacts.canonical[b]);
            if a==b || wire.and_then(|w|labels.get(&w)).is_some_and(|label|label!=group) {return None}
            Some((a,b,length,wire))
        }).collect();
        let mut graph=vec![Vec::new();size];
        for (index,&(a,b,_,_)) in allowed.iter().enumerate() {graph[a].push((b,index));graph[b].push((a,index));}
        let witnesses:Vec<_>=allowed.iter().enumerate().filter(|(_,s)|s.3.and_then(|w|labels.get(&w))==Some(group))
            .map(|(index,s)|(index,reached(&graph,&blocked,s.0,index),reached(&graph,&blocked,s.1,index))).collect();
        for (pair,a,b,_) in pairs.iter().filter(|(_,_,_,represented)|represented.as_ref()!=Some(group)) {
            let best=witnesses.iter().filter(|(_,left,right)|left.contains(a) && right.contains(b) || left.contains(b) && right.contains(a))
                .filter_map(|(witness,_,_)|through(size,&allowed,*a,*b,*witness,&blocked)).min();
            if let Some((_,route))=best {
                paths.pairs.push(*pair);paths.wire_routes.push(route);
            }
        }
    }
}

#[cfg(test)]
mod tests;
