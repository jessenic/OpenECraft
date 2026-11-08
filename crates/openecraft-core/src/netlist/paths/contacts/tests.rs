use super::*;
use crate::geom::p;
use crate::model::Wire;

fn attachment(symbol:u64,point:P)->Attachment {
    Attachment {page:0,point,symbol,pin:0,mask:15,side_contact:false,name:None,bridges:Vec::new()}
}

fn fixture()->(Project,Net,Vec<Attachment>) {
    let mut prj=Project::default();
    prj.pages[0].values.insert("page.explicit_wire_junctions".into(),"true".into());
    prj.pages[0].wires=vec![
        Wire {id:10,a:p(0.,0.),b:p(20.,0.),..Default::default()},
        Wire {id:11,a:p(10.,0.),b:p(10.,10.),..Default::default()},
        Wire {id:12,a:p(20.,0.),b:p(20.,10.),..Default::default()},
        Wire {id:13,a:p(20.,10.),b:p(10.,10.),..Default::default()},
    ];
    let net=Net {wires:(10..14).map(|id|(0,id)).collect(),..Default::default()};
    (prj,net,vec![attachment(1,p(0.,0.)),attachment(2,p(10.,10.))])
}

#[test]
fn one_electrical_net_does_not_allow_a_shortcut_at_an_unjoined_crossing() {
    let (prj,net,pins)=fixture();
    let graph=build(&prj,&net,&[],&pins);
    let route=wire_route(&graph_of(graph.uf.parent.len(),&graph.edges),graph.nodes[&pins[0].key()],graph.nodes[&pins[1].key()]);
    assert_eq!(route,vec![(0,10),(0,12),(0,13)]);
}

#[test]
fn traditional_pages_allow_an_endpoint_to_join_a_segment_interior() {
    let (mut prj,net,pins)=fixture();
    prj.pages[0].values.remove("page.explicit_wire_junctions");
    let graph=build(&prj,&net,&[],&pins);
    let route=wire_route(&graph_of(graph.uf.parent.len(),&graph.edges),graph.nodes[&pins[0].key()],graph.nodes[&pins[1].key()]);
    assert_eq!(route,vec![(0,10),(0,11)]);
}

#[test]
fn a_side_attachment_requires_both_permission_and_an_accepted_direction() {
    let (prj,net,mut pins)=fixture();
    pins[0].point=p(5.,0.);pins[0].mask=1;
    for (side,mask,connected) in [(false,15,false),(true,1,false),(true,2,true)] {
        pins[0].side_contact=side;pins[0].mask=mask;
        let mut graph=build(&prj,&net,&[],&pins);
        assert_eq!(graph.uf.find(graph.nodes[&pins[0].key()])==graph.uf.find(graph.nodes[&pins[1].key()]),connected);
    }
}
