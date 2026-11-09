//! Configuration and physical pin identity of channels in complete PLC module symbols.
use super::*;
use crate::symbols::{PlcChannelDef, SymbolDef};

pub use crate::model::plc_hw::bindings::{key,set_value};

pub fn value(s:&SymbolInst,field:&str,index:usize)->String {
    s.values.get(&key(field,index)).cloned().unwrap_or_default()
}

pub fn set(prj:&mut Project,id:u64,index:usize,field:&str,value:&str)->Result<bool,String> {
    crate::model::plc_hw::bindings::set(prj,id,index,field,value)
}

pub(super) fn logical_points(prj:&Project,rows:Vec<Point>)->Vec<Point> {
    let mut result=Vec::new();let mut groups=BTreeMap::<(String,String),Vec<Point>>::new();
    for point in rows {
        let symbol=&prj.pages[point.page].symbols[point.sym];
        if let Some(group)=point.channel_index.and_then(|index|crate::model::plc_hw::bindings::group(symbol,index)) {groups.entry(group).or_default().push(point);}
        else {result.push(point);}
    }
    for mut group in groups.into_values() {
        group.sort_by_key(|p|(!prj.pages[p.page].symbols[p.sym].plc_bindings.get(&p.channel_index.unwrap()).is_some_and(|b|b.primary),p.id,p.channel_index));
        let mut point=group[0].clone();
        for other in group.iter().skip(1) {
            if point.address.is_empty() {point.address=other.address.clone();}
            if point.symbolic.is_empty() {point.symbolic=other.symbolic.clone();}
            if point.function_text.is_empty() {point.function_text=other.function_text.clone();}
        }
        let index=point.channel_index.unwrap();
        if let Some(number)=crate::model::plc_hw::bindings::index(prj,point.id,index) {point.channel=number.to_string();}
        point.address=crate::model::plc_hw::bindings::value(prj,point.id,index,"address").unwrap_or(point.address);
        point.symbolic=crate::model::plc_hw::bindings::value(prj,point.id,index,"symbol").unwrap_or(point.symbolic);
        point.function_text=crate::model::plc_hw::bindings::value(prj,point.id,index,"comment").map(|v|v.replace('\n'," ")).unwrap_or(point.function_text);
        result.push(point);
    }
    result.sort_by(|a,b|natural_cmp(&a.card,&b.card).then_with(||address_key(&a.address).cmp(&address_key(&b.address))));
    result
}

fn connected(prj:&Project,nl:&Netlist,pi:usize,si:usize,pins:&[usize],cache:&mut HashMap<usize,Vec<(String,PinAt)>>)->String {
    let s=&prj.pages[pi].symbols[si];let mut targets=Vec::new();
    for pin in pins {
        let Some(&net)=nl.pin_net.get(&(s.id,*pin)) else {continue;};
        let list=cache.entry(net).or_insert_with(|| {
            let mut list:Vec<_>=nl.nets[net].pins.iter().map(|p|(pin_designation(prj,*p),(p.page,p.sym))).collect();
            list.sort_by(|a,b|natural_cmp(&a.0,&b.0));list
        });
        targets.extend(list.iter().filter(|(_,at)|*at!=(pi,si)).map(|(text,_)|text.clone()));
    }
    targets.sort_by(|a,b|natural_cmp(a,b));targets.dedup();
    let extra=targets.len().saturating_sub(MAX_TARGETS);targets.truncate(MAX_TARGETS);
    let mut text=targets.join(", ");if extra>0 {text.push_str(&format!(", … (+{extra})"));}text
}

pub(super) fn points(prj:&Project,nl:&Netlist,pi:usize,si:usize,def:&SymbolDef,cache:&mut HashMap<usize,Vec<(String,PinAt)>>)->Vec<Point> {
    let s=&prj.pages[pi].symbols[si];
    def.plc_channels.iter().enumerate().map(|(index,c)| {
        let pins=c.signal_pin.map(|p|vec![p]).unwrap_or_else(||c.pins.clone());
        let terminal=pins.iter().filter_map(|p|s.pins.get(*p).map(String::as_str).or_else(||def.pins.get(*p).map(|p|p.label))).collect::<Vec<_>>().join(", ");
        let configured_type=value(s,"data_type",index);
        let data_type=if configured_type.is_empty() {c.data_type.clone()} else {configured_type};
        let position=pins.first().and_then(|p|def.pins.get(*p).map(|_|def.pin_world(*p,s.pos,s.rot,s.mirror))).unwrap_or(s.pos);
        Point {page:pi,sym:si,id:s.id,card:s.tag.clone(),address:value(s,"address",index),input:c.input.unwrap_or(false),
            analog_type:c.analog.then_some(data_type.clone()),data_type,channel:c.designation.clone(),terminal,
            function_text:value(s,"comment",index).replace('\n'," "),connected:connected(prj,nl,pi,si,&pins,cache),
            location:crate::frame::reference(prj,pi,position),channel_index:Some(index),symbolic:value(s,"symbol",index),direction:c.input}
    }).collect()
}

pub fn definitions(prj:&Project,id:u64)->Vec<PlcChannelDef> {
    prj.pages.iter().flat_map(|p|&p.symbols).find(|s|s.id==id).and_then(|s|prj.symbol(&s.sym)).map(|d|d.plc_channels.clone()).unwrap_or_default()
}

/// Convert configured digital addresses without resetting their assigned offsets.
pub fn convert_addresses(prj:&mut Project)->usize {
    let mut updates=Vec::new();
    for (pi,si,index) in crate::model::plc_hw::bindings::placed(prj) {
        let s=&prj.pages[pi].symbols[si];let Some(def)=prj.symbol(&s.sym) else {continue;};let c=&def.plc_channels[index];
        if c.analog || c.input.is_none() {continue;}
        let old=crate::model::plc_hw::bindings::value(prj,s.id,index,"address").unwrap_or_default();
        let Some(address)=crate::plc_hw::parse_address(&old).filter(|a|Some(a.input)==c.input) else {continue;};
        let data=prj.plc_cards.get(&s.tag).cloned().unwrap_or_default();
        let start=if address.input {data.start_byte} else {data.output_byte.unwrap_or(data.start_byte)};
        let channel=if address.slot.is_some() {address.byte*8+address.bit} else {address.byte.saturating_sub(start)*8+address.bit};
        let new=crate::plc_hw::card_address(prj,&s.tag,address.input,channel);
        if old!=new {updates.push((s.id,index,new));}
    }
    updates.into_iter().filter(|(id,index,address)|set(prj,*id,*index,"address",address).unwrap_or(false)).count()
}

fn same_address(a:&str,b:&str)->bool {
    a.trim().trim_start_matches('%').eq_ignore_ascii_case(b.trim().trim_start_matches('%'))
}

struct Addressed {
    id:u64,
    index:usize,
    card:String,
    address:String,
}

/// The logical channels with an address (a linked channel once).
fn addressed(prj:&Project)->Vec<Addressed> {
    crate::model::plc_hw::bindings::placed(prj).into_iter().filter_map(|(pi,si,index)| {
        let s=&prj.pages[pi].symbols[si];
        let mut address=value(s,"address",index);
        if address.is_empty() && crate::model::plc_hw::bindings::group(s,index).is_some() {
            address=crate::model::plc_hw::bindings::value(prj,s.id,index,"address").unwrap_or_default();
        }
        (!address.is_empty()).then(||Addressed {id:s.id,index,card:s.tag.clone(),address})
    }).collect()
}

pub(super) fn apply_texts(prj:&mut Project,domain:Option<&str>,texts:&[(String,String)])->usize {
    let mut updates=Vec::new();
    for point in addressed(prj) {
        if domain.is_some_and(|d|crate::plc_hw::card_domain(prj,&point.card)!=d) {continue;}
        if let Some((_,text))=texts.iter().find(|(a,_)|same_address(a,&point.address)) {
            updates.push((point.id,point.index,text.strip_prefix(&format!("{} ",point.card)).unwrap_or(text).to_owned()));
        }
    }
    updates.into_iter().filter(|(id,index,text)|set(prj,*id,*index,"comment",text).unwrap_or(false)).count()
}

pub(super) fn import_rows(prj:&mut Project,rows:&[(String,String,String)])->usize {
    let mut updates=Vec::new();
    for point in addressed(prj) {
        if let Some((_,name,comment))=rows.iter().find(|(a,_,_)|same_address(a,&point.address)) {
            updates.push((point.id,point.index,name.clone(),comment.strip_prefix(&format!("{} ",point.card)).unwrap_or(comment).to_owned()));
        }
    }
    updates.into_iter().filter(|(id,index,name,comment)| {
        let a=set(prj,*id,*index,"symbol",name).unwrap_or(false);
        let b=set(prj,*id,*index,"comment",comment).unwrap_or(false);a||b
    }).count()
}

pub(super) fn readdress(prj:&mut Project,card:&str)->usize {
    let symbols=prj.symbol_table();let mut updates=Vec::new();let mut seen=HashSet::new();
    for page in prj.pages.iter().filter(|p|p.kind==PageKind::Schematic) {
        for s in page.symbols.iter().filter(|s|s.tag==card) {
            let Some(def)=symbols.get(&s.sym).filter(|d|!matches!(d.func,Func::Overview|Func::Graphic)) else {continue;};
            for (index,c) in def.plc_channels.iter().enumerate().filter(|(_,c)|!c.analog) {
                if crate::model::plc_hw::bindings::group(s,index).is_some_and(|group|!seen.insert(group)) {continue;}
                if let Some(input)=c.input {
                    let number=crate::model::plc_hw::bindings::index(prj,s.id,index).unwrap_or(c.index);
                    updates.push((s.id,index,crate::plc_hw::card_address(prj,card,input,u32::from(number))));
                }
            }
        }
    }
    updates.into_iter().filter(|(id,index,address)|set(prj,*id,*index,"address",address).unwrap_or(false)).count()
}

#[cfg(test)]
mod tests;
