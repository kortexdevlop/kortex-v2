//! Rules of the canvas model, against a small catalogue.
use std::collections::BTreeMap;

use kortex_contract::PortType;
use kortex_flow::canvas::{
    History, Refusal, View, add_node, connect, fit, link_path, node_ports, port_point, remove_node,
};
use kortex_flow::engine::{Endpoint, NodeKind, Port, ToolPorts, Workflow};

fn catalogue() -> BTreeMap<String, ToolPorts> {
    let port = |id: &str, ty: PortType, required: bool| Port {
        id: id.into(),
        ty,
        required,
    };
    BTreeMap::from([
        (
            "t.upper".into(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![],
                outputs: vec![port("text", PortType::Text, false)],
            },
        ),
        (
            "t.table".into(),
            ToolPorts {
                inputs: vec![port("text", PortType::Text, true)],
                params: vec![],
                outputs: vec![port("table", PortType::Table, false)],
            },
        ),
    ])
}

fn tool(id: &str) -> NodeKind {
    NodeKind::Tool {
        tool: id.into(),
        params: BTreeMap::new(),
    }
}

fn end(node: &str, port: &str) -> Endpoint {
    Endpoint {
        node: node.into(),
        port: port.into(),
    }
}

#[test]
fn adding_after_a_node_chains_it_one_column_to_the_right() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let a = add_node(
        &mut w,
        &cat,
        NodeKind::Text { value: "x".into() },
        None,
        None,
    );
    let b = add_node(&mut w, &cat, tool("t.upper"), Some(&a), None);
    let c = add_node(&mut w, &cat, tool("t.upper"), Some(&b), None);
    assert_eq!(w.edges.len(), 2);
    assert_eq!(w.edges[0].from, end(&a, "text"));
    assert_eq!(w.edges[0].to, end(&b, "text"));
    let x = |id: &str| {
        w.nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.pos)
            .map(|p| p.x)
    };
    assert!(x(&a) < x(&b) && x(&b) < x(&c));
    let d = add_node(&mut w, &cat, tool("t.upper"), Some(&a), None);
    let y = |id: &str| {
        w.nodes
            .iter()
            .find(|n| n.id == id)
            .and_then(|n| n.pos)
            .map(|p| p.y)
    };
    assert_ne!(y(&b), y(&d), "a second branch does not overlap the first");
}

#[test]
fn connections_are_checked_for_nodes_types_and_cycles() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let a = add_node(
        &mut w,
        &cat,
        NodeKind::Text {
            value: String::new(),
        },
        None,
        None,
    );
    let b = add_node(&mut w, &cat, tool("t.upper"), Some(&a), None);
    let t = add_node(&mut w, &cat, tool("t.table"), Some(&b), None);
    assert_eq!(
        connect(&mut w, &cat, &end(&b, "text"), &end(&b, "text")),
        Err(Refusal::SameNode)
    );
    assert_eq!(
        connect(&mut w, &cat, &end(&t, "table"), &end(&b, "text")),
        Err(Refusal::Incompatible)
    );
    assert_eq!(
        connect(&mut w, &cat, &end(&t, "nope"), &end(&b, "text")),
        Err(Refusal::Unknown)
    );
    // b feeds t; t feeding back into b through text would need a table→text conversion, so use a
    // text-only loop: t.upper → t.upper.
    let c = add_node(&mut w, &cat, tool("t.upper"), Some(&b), None);
    assert_eq!(
        connect(&mut w, &cat, &end(&c, "text"), &end(&b, "text")),
        Err(Refusal::Cycle)
    );
    let before = w.clone();
    assert_eq!(
        connect(&mut w, &cat, &end(&a, "text"), &end(&c, "text")),
        Ok(())
    );
    assert_eq!(
        w.edges.len(),
        before.edges.len(),
        "an input keeps a single link"
    );
}

#[test]
fn removing_a_node_removes_its_links() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let a = add_node(
        &mut w,
        &cat,
        NodeKind::Text {
            value: String::new(),
        },
        None,
        None,
    );
    let b = add_node(&mut w, &cat, tool("t.upper"), Some(&a), None);
    remove_node(&mut w, &b);
    assert!(w.edges.is_empty());
    assert_eq!(w.nodes.len(), 1);
}

#[test]
fn ports_sit_on_the_card_edges_and_links_are_horizontal_curves() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let a = add_node(
        &mut w,
        &cat,
        NodeKind::Text {
            value: String::new(),
        },
        None,
        None,
    );
    let b = add_node(&mut w, &cat, tool("t.upper"), Some(&a), None);
    let (na, nb) = (&w.nodes[0], &w.nodes[1]);
    let out = port_point(na, &cat, true, "text").expect("output");
    let input = port_point(nb, &cat, false, "text").expect("input");
    assert!(out.0 < input.0);
    assert!(port_point(na, &cat, false, "text").is_none());
    assert_eq!(node_ports(nb, &cat).0.len(), 1);
    assert!(link_path(out, input).starts_with('M'));
    assert_eq!(b, "n2");
}

#[test]
fn zoom_keeps_the_point_under_the_cursor_and_is_clamped() {
    let v = View::default();
    let z = v.zoomed(300.0, 200.0, 1.5);
    let before = v.to_canvas(300.0, 200.0);
    let after = z.to_canvas(300.0, 200.0);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
    let mut far = v;
    for _ in 0..50 {
        far = far.zoomed(0.0, 0.0, 2.0);
    }
    assert!(far.zoom <= 2.0);
    let pan = v.panned(10.0, -5.0);
    assert_eq!((pan.x, pan.y), (10.0, -5.0));
}

#[test]
fn fit_shows_every_node_inside_the_viewport() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let mut last = add_node(
        &mut w,
        &cat,
        NodeKind::Text {
            value: String::new(),
        },
        None,
        None,
    );
    for _ in 0..6 {
        last = add_node(&mut w, &cat, tool("t.upper"), Some(&last), None);
    }
    let view = fit(&w, &cat, 800.0, 500.0);
    for node in &w.nodes {
        let p = node.pos.expect("placed");
        let (sx, sy) = (
            f64::from(p.x) * view.zoom + view.x,
            f64::from(p.y) * view.zoom + view.y,
        );
        assert!((0.0..=800.0).contains(&sx) && (0.0..=500.0).contains(&sy));
    }
    assert_eq!(
        fit(&Workflow::default(), &cat, 800.0, 500.0),
        View::default()
    );
}

#[test]
fn history_undoes_redoes_and_drops_the_redo_branch_on_a_new_change() {
    let cat = catalogue();
    let mut w = Workflow::default();
    let mut h = History::default();
    h.record(&w);
    add_node(
        &mut w,
        &cat,
        NodeKind::Text {
            value: String::new(),
        },
        None,
        None,
    );
    let one = w.clone();
    h.record(&w);
    add_node(&mut w, &cat, tool("t.upper"), None, None);
    let w2 = h.undo(&w).expect("undo");
    assert_eq!(w2, one);
    assert!(h.can_redo());
    let w3 = h.redo(&w2).expect("redo");
    assert_eq!(w3.nodes.len(), 2);
    let back = h.undo(&w3).expect("undo again");
    h.record(&back);
    assert!(!h.can_redo(), "a new change ends the redo branch");
}
