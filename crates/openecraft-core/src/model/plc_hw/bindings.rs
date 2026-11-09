//! Shared instance values for logical PLC channels with several graphical views.
use crate::model::{Project,SymbolInst};
use openecraft_i18n::{tr,tr_fmt};

pub fn key(field:&str,index:usize)->String {format!("function.plc_{field}[{}]",index+1)}

pub fn set_value(s:&mut SymbolInst,field:&str,index:usize,value:&str)->bool {
    let key=key(field,index);
    if s.values.get(&key).map(String::as_str).unwrap_or_default()==value {return false;}
    if value.is_empty() {s.values.remove(&key);} else {s.values.insert(key,value.into());}
    true
}

pub fn property(key:&str)->Option<(&str,usize)> {
    let rest=key.strip_prefix("function.plc_")?;
    let (field,index)=rest.strip_suffix(']')?.split_once('[')?;
    if !["address","symbol","comment","data_type"].contains(&field) {return None;}
    Some((field,index.parse::<usize>().ok()?.checked_sub(1)?))
}

fn source(prj:&Project,id:u64,index:usize)->Option<&SymbolInst> {
    let symbol=prj.pages.iter().flat_map(|p|&p.symbols).find(|s|s.id==id)?;
    (index<prj.symbol(&symbol.sym)?.plc_channels.len()).then_some(symbol)
}

pub fn group(symbol:&SymbolInst,index:usize)->Option<(String,String)> {
    symbol.plc_bindings.get(&index).filter(|b|!b.group.is_empty()).map(|b|(symbol.tag.clone(),b.group.clone()))
}

/// Active schematic representations, counting linked module channels once.
pub fn placed(prj:&Project)->Vec<(usize,usize,usize)> {
    placed_for(prj,None)
}

pub fn placed_for(prj:&Project,tag:Option<&str>)->Vec<(usize,usize,usize)> {
    let mut result=Vec::new();let mut groups=std::collections::BTreeMap::new();
    for (pi,page) in prj.pages.iter().enumerate().filter(|(_,p)|p.kind==crate::model::PageKind::Schematic) {
        for (si,s) in page.symbols.iter().enumerate().filter(|(_,s)|tag.is_none_or(|tag|s.tag==tag)) {
            let Some(def)=prj.symbol(&s.sym).filter(|d|!matches!(d.func,crate::symbols::Func::Overview|crate::symbols::Func::Graphic)) else {continue;};
            for index in 0..def.plc_channels.len() {
                let at=(pi,si,index);
                if let Some(group)=group(s,index) {
                    let priority=(!s.plc_bindings[&index].primary,s.id,index);
                    let entry=groups.entry(group).or_insert((priority,at));
                    if priority<entry.0 {*entry=(priority,at);}
                } else {result.push(at);}
            }
        }
    }
    result.extend(groups.into_values().map(|(_,at)|at));result
}

pub fn index(prj:&Project,id:u64,index:usize)->Option<u16> {
    let symbol=source(prj,id,index)?;
    if let Some(index)=symbol.plc_bindings.get(&index).and_then(|b|b.index) {return Some(index);}
    let mut candidates=references(prj,id,index);
    candidates.sort_by_key(|(pi,si,index)| {
        let s=&prj.pages[*pi].symbols[*si];
        (std::cmp::Reverse(prj.symbol(&s.sym).map(|d|d.plc_channels.len()).unwrap_or(0)),!s.plc_bindings.get(index).is_some_and(|b|b.primary),s.id,*index)
    });
    let (pi,si,index)=*candidates.first()?;
    prj.symbol(&prj.pages[pi].symbols[si].sym)?.plc_channels.get(index).map(|c|c.index)
}

pub fn references(prj:&Project,id:u64,index:usize)->Vec<(usize,usize,usize)> {
    let Some(symbol)=source(prj,id,index) else {return Vec::new();};
    let binding=group(symbol,index);
    let mut result=Vec::new();
    for (pi,page) in prj.pages.iter().enumerate() {
        for (si,s) in page.symbols.iter().enumerate() {
            if s.id==id && binding.is_none() {result.push((pi,si,index));}
            else if let Some((tag,group))=&binding {
                if &s.tag==tag {
                    result.extend(s.plc_bindings.iter().filter(|(_,b)|&b.group==group).map(|(index,_)|(pi,si,*index)));
                }
            }
        }
    }
    result.sort_by_key(|(pi,si,index)|!prj.pages[*pi].symbols[*si].plc_bindings.get(index).is_some_and(|b|b.primary));
    result
}

pub fn value(prj:&Project,id:u64,index:usize,field:&str)->Option<String> {
    let symbol=source(prj,id,index)?;
    let value=symbol.values.get(&key(field,index)).filter(|v|!v.is_empty());
    if let Some(value)=value {return Some(value.clone());}
    if symbol.plc_bindings.get(&index).is_none_or(|b|b.group.is_empty()) {return Some(String::new());}
    Some(references(prj,id,index).into_iter().find_map(|(pi,si,index)|prj.pages[pi].symbols[si].values.get(&key(field,index)).filter(|v|!v.is_empty()).cloned()).unwrap_or_default())
}

pub fn set(prj:&mut Project,id:u64,index:usize,field:&str,value:&str)->Result<bool,String> {
    if !["address","symbol","comment","data_type"].contains(&field) {return Err(tr_fmt!("Unknown property: {}",field));}
    let references=references(prj,id,index);
    if references.is_empty() {return Err(tr_fmt!("No connection point {}",format!("{id}:{index}")));}
    if references.iter().any(|(pi,_,_)|prj.pages[*pi].locked) {return Err(tr("The row lies on a released page").into());}
    let mut changed=false;
    for (pi,si,index) in references {
        changed|=set_value(&mut prj.pages[pi].symbols[si],field,index,value);
    }
    Ok(changed)
}
