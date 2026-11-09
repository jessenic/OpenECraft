use super::*;

#[test]
fn candidate_ends_stop_at_physical_terminals_and_exclude_the_witness() {
    let graph=vec![vec![(1,0)],vec![(0,0),(2,1),(3,2)],vec![(1,1),(4,3)],vec![(1,2)],vec![(2,3)]];
    let physical=HashSet::from([0,2,3,4]);
    assert_eq!(reached(&graph,&physical,1,0),HashSet::from([2,3]));
    assert_eq!(reached(&graph,&physical,0,0),HashSet::from([0]));
}

#[test]
fn longer_parallel_core_is_retained() {
    let edges=[(0,1,1,Some((0,1))),(0,2,2,Some((0,2))),(2,3,2,Some((0,3))),(3,1,2,Some((0,4)))];
    assert_eq!(through(4,&edges,0,1,2,&HashSet::new()),Some((6,vec![(0,2),(0,3),(0,4)])));
}

#[test]
fn an_out_and_back_spur_cannot_be_a_conductor() {
    let edges=[(0,1,1,Some((0,1))),(1,2,1,Some((0,2))),(2,3,1,Some((0,3)))];
    assert!(through(4,&edges,0,1,2,&HashSet::new()).is_none());
}

#[test]
fn residual_paths_can_undo_the_first_greedy_choice() {
    let edges=[(0,2,1,Some((0,1))),(1,2,1,Some((0,2))),(2,3,1,Some((0,3))),
               (2,4,2,Some((0,4))),(0,5,2,Some((0,5))),(5,3,2,Some((0,6))),
               (3,4,1,Some((0,7)))];
    let (cost,wires)=through(6,&edges,0,1,6,&HashSet::new()).unwrap();
    assert_eq!(cost,8);assert_eq!(wires,vec![(0,5),(0,6),(0,7),(0,4),(0,2)]);
    assert!(through(6,&edges,0,1,6,&HashSet::from([5])).is_none());
}
