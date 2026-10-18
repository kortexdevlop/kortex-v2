//! Pure model of the Flow canvas: node geometry, ports, links, the view (pan and zoom), editing
//! rules (connect, chain, move, remove) and undo/redo. No UI here, so every rule is unit-tested.
use std::collections::BTreeMap;

use kortex_contract::PortType;

use crate::engine::{
    Catalogue, Edge, Endpoint, LAYOUT_STEP, Node, NodeKind, Pos, TEXT_PORT, Workflow, order,
};

pub const NODE_W: f64 = 232.0;
pub const HEADER_H: f64 = 38.0;
pub const ROW_H: f64 = 26.0;
pub const BODY_PAD: f64 = 10.0;
pub const MIN_ZOOM: f64 = 0.25;
pub const MAX_ZOOM: f64 = 2.0;

/// One port of a node as the canvas draws it.
#[derive(Debug, Clone, PartialEq)]
pub struct PortView {
    pub id: String,
    pub ty: PortType,
    pub required: bool,
}

/// Input and output ports of a node, from the tool contract (a text node has one output).
#[must_use]
pub fn node_ports(node: &Node, catalogue: &dyn Catalogue) -> (Vec<PortView>, Vec<PortView>) {
    match &node.kind {
        NodeKind::Text { .. } => (
            Vec::new(),
            vec![PortView {
                id: TEXT_PORT.to_owned(),
                ty: PortType::Text,
                required: false,
            }],
        ),
        NodeKind::Tool { tool, .. } => catalogue.tool(tool).map_or_else(
            || (Vec::new(), Vec::new()),
            |p| {
                let view = |port: &crate::engine::Port| PortView {
                    id: port.id.clone(),
                    ty: port.ty.clone(),
                    required: port.required,
                };
                (
                    p.inputs.iter().map(view).collect(),
                    p.outputs.iter().map(view).collect(),
                )
            },
        ),
    }
}

/// Height of a node card: header, one row per port on the busiest side, and a body for text.
#[must_use]
pub fn node_height(node: &Node, catalogue: &dyn Catalogue) -> f64 {
    let (inputs, outputs) = node_ports(node, catalogue);
    let rows = inputs.len().max(outputs.len()).max(1);
    let body = if matches!(node.kind, NodeKind::Text { .. }) {
        34.0
    } else {
        0.0
    };
    HEADER_H + BODY_PAD + rows as f64 * ROW_H + body
}

fn origin(node: &Node) -> (f64, f64) {
    node.pos
        .map_or((0.0, 0.0), |p| (f64::from(p.x), f64::from(p.y)))
}

/// Centre of a port handle in canvas units: inputs on the left edge, outputs on the right edge.
#[must_use]
pub fn port_point(
    node: &Node,
    catalogue: &dyn Catalogue,
    output: bool,
    port: &str,
) -> Option<(f64, f64)> {
    let (inputs, outputs) = node_ports(node, catalogue);
    let list = if output { outputs } else { inputs };
    let index = list.iter().position(|p| p.id == port)?;
    let (x, y) = origin(node);
    Some((
        if output { x + NODE_W } else { x },
        y + HEADER_H + BODY_PAD + index as f64 * ROW_H + ROW_H / 2.0,
    ))
}

/// Cubic curve between an output and an input handle; it leaves and enters horizontally.
#[must_use]
pub fn link_path(from: (f64, f64), to: (f64, f64)) -> String {
    let reach = ((to.0 - from.0).abs() * 0.5).max(48.0);
    format!(
        "M{:.1} {:.1} C{:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
        from.0,
        from.1,
        from.0 + reach,
        from.1,
        to.0 - reach,
        to.1,
        to.0,
        to.1
    )
}

/// The visible window on the canvas: translation in screen pixels and a zoom factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

impl Default for View {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl View {
    /// Canvas point under a screen point.
    #[must_use]
    pub fn to_canvas(&self, sx: f64, sy: f64) -> (f64, f64) {
        ((sx - self.x) / self.zoom, (sy - self.y) / self.zoom)
    }

    /// Zooms by `factor` keeping the canvas point under (`sx`, `sy`) fixed.
    #[must_use]
    pub fn zoomed(&self, sx: f64, sy: f64, factor: f64) -> Self {
        let zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let (cx, cy) = self.to_canvas(sx, sy);
        Self {
            x: sx - cx * zoom,
            y: sy - cy * zoom,
            zoom,
        }
    }

    #[must_use]
    pub fn panned(&self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            zoom: self.zoom,
        }
    }
}

/// Bounding box of every node: (min x, min y, max x, max y), or `None` for an empty canvas.
#[must_use]
pub fn bounds(workflow: &Workflow, catalogue: &dyn Catalogue) -> Option<(f64, f64, f64, f64)> {
    workflow.nodes.iter().fold(None, |acc, node| {
        let (x, y) = origin(node);
        let (x2, y2) = (x + NODE_W, y + node_height(node, catalogue));
        Some(acc.map_or((x, y, x2, y2), |(a, b, c, d)| {
            (a.min(x), b.min(y), c.max(x2), d.max(y2))
        }))
    })
}

/// A view showing the whole workflow in a viewport, never zoomed in past 100 %.
#[must_use]
pub fn fit(workflow: &Workflow, catalogue: &dyn Catalogue, width: f64, height: f64) -> View {
    let Some((x1, y1, x2, y2)) = bounds(workflow, catalogue) else {
        return View::default();
    };
    let margin = 48.0;
    let zoom = ((width - 2.0 * margin) / (x2 - x1))
        .min((height - 2.0 * margin) / (y2 - y1))
        .clamp(MIN_ZOOM, 1.0);
    View {
        x: (width - (x2 - x1) * zoom) / 2.0 - x1 * zoom,
        y: (height - (y2 - y1) * zoom) / 2.0 - y1 * zoom,
        zoom,
    }
}

/// Why a connection was refused (translated by the UI as `canvas.<code>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    SameNode,
    Unknown,
    Incompatible,
    Cycle,
}

impl Refusal {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::SameNode => "same_node",
            Self::Unknown => "unknown_port",
            Self::Incompatible => "incompatible",
            Self::Cycle => "cycle",
        }
    }
}

fn find<'a>(workflow: &'a Workflow, id: &str) -> Option<&'a Node> {
    workflow.nodes.iter().find(|n| n.id == id)
}

/// Checks a link from an output to an input without changing anything.
///
/// # Errors
/// The reason the link is not allowed.
pub fn can_connect(
    workflow: &Workflow,
    catalogue: &dyn Catalogue,
    from: &Endpoint,
    to: &Endpoint,
) -> Result<(), Refusal> {
    if from.node == to.node {
        return Err(Refusal::SameNode);
    }
    let (Some(a), Some(b)) = (find(workflow, &from.node), find(workflow, &to.node)) else {
        return Err(Refusal::Unknown);
    };
    let out = node_ports(a, catalogue)
        .1
        .into_iter()
        .find(|p| p.id == from.port);
    let input = node_ports(b, catalogue)
        .0
        .into_iter()
        .find(|p| p.id == to.port);
    let (Some(out), Some(input)) = (out, input) else {
        return Err(Refusal::Unknown);
    };
    if !out.ty.converts_to(&input.ty) {
        return Err(Refusal::Incompatible);
    }
    let mut trial = workflow.clone();
    set_edge(&mut trial, to, Some(from.clone()));
    order(&trial).map(|_| ()).map_err(|_| Refusal::Cycle)
}

/// Replaces whatever fed `to` (an input takes one link) by `from`, or clears it.
pub fn set_edge(workflow: &mut Workflow, to: &Endpoint, from: Option<Endpoint>) {
    workflow.edges.retain(|e| &e.to != to);
    if let Some(from) = from {
        workflow.edges.push(Edge {
            from,
            to: to.clone(),
        });
    }
}

/// Connects after checking the rules.
///
/// # Errors
/// The reason the link is not allowed; the workflow is then untouched.
pub fn connect(
    workflow: &mut Workflow,
    catalogue: &dyn Catalogue,
    from: &Endpoint,
    to: &Endpoint,
) -> Result<(), Refusal> {
    can_connect(workflow, catalogue, from, to)?;
    set_edge(workflow, to, Some(from.clone()));
    Ok(())
}

/// Removes a node and every link touching it.
pub fn remove_node(workflow: &mut Workflow, id: &str) {
    workflow.nodes.retain(|n| n.id != id);
    workflow
        .edges
        .retain(|e| e.from.node != id && e.to.node != id);
}

#[must_use]
pub fn next_id(workflow: &Workflow) -> String {
    (1..)
        .map(|n| format!("n{n}"))
        .find(|id| workflow.nodes.iter().all(|node| &node.id != id))
        .unwrap_or_default()
}

pub fn move_node(workflow: &mut Workflow, id: &str, x: i32, y: i32) {
    if let Some(node) = workflow.nodes.iter_mut().find(|n| n.id == id) {
        node.pos = Some(Pos { x, y });
    }
}

/// Adds a node. With a selected node it goes one column to the right and is linked to it by the
/// first compatible pair of ports (the "chain" gesture); otherwise it goes at `at`, or in the
/// first free place of the layout. Returns the new node's id.
pub fn add_node(
    workflow: &mut Workflow,
    catalogue: &dyn Catalogue,
    kind: NodeKind,
    after: Option<&str>,
    at: Option<(i32, i32)>,
) -> String {
    let id = next_id(workflow);
    let anchor = after.and_then(|a| find(workflow, a)).cloned();
    let pos = match (&anchor, at) {
        (_, Some((x, y))) => Pos { x, y },
        (Some(a), None) => {
            let p = a.pos.unwrap_or(Pos { x: 40, y: 40 });
            let mut y = p.y;
            // Stay clear of nodes already placed in that column.
            while workflow.nodes.iter().any(|n| {
                n.pos
                    .is_some_and(|q| q.x == p.x + LAYOUT_STEP.0 && (q.y - y).abs() < 120)
            }) {
                y += LAYOUT_STEP.1;
            }
            Pos {
                x: p.x + LAYOUT_STEP.0,
                y,
            }
        }
        (None, None) => {
            let free = workflow
                .nodes
                .iter()
                .filter_map(|n| n.pos)
                .fold((40, 40), |(x, y), p| {
                    (
                        x.max(p.x + LAYOUT_STEP.0),
                        if p.x + LAYOUT_STEP.0 > x { p.y } else { y },
                    )
                });
            let first = workflow.nodes.is_empty();
            Pos {
                x: if first { 40 } else { free.0 },
                y: if first { 40 } else { free.1 },
            }
        }
    };
    workflow.nodes.push(Node {
        id: id.clone(),
        kind,
        retries: 0,
        pos: Some(pos),
    });
    if let Some(anchor) = anchor {
        let new = workflow.nodes.last().cloned();
        if let Some(new) = new {
            let outs = node_ports(&anchor, catalogue).1;
            let ins = node_ports(&new, catalogue).0;
            // Required inputs first: that is what makes the chain runnable.
            let mut ins = ins;
            ins.sort_by_key(|p| !p.required);
            let pair = ins.iter().find_map(|i| {
                outs.iter()
                    .find(|o| o.ty.converts_to(&i.ty))
                    .map(|o| (o.id.clone(), i.id.clone()))
            });
            if let Some((out, input)) = pair {
                set_edge(
                    workflow,
                    &Endpoint {
                        node: id.clone(),
                        port: input,
                    },
                    Some(Endpoint {
                        node: anchor.id.clone(),
                        port: out,
                    }),
                );
            }
        }
    }
    id
}

/// Bounded undo/redo over whole-workflow snapshots (workflows are small).
#[derive(Debug, Clone, Default)]
pub struct History {
    past: Vec<Workflow>,
    future: Vec<Workflow>,
}

const HISTORY_LIMIT: usize = 100;

impl History {
    /// Records the state *before* a change; a new change drops the redo branch.
    pub fn record(&mut self, before: &Workflow) {
        if self.past.last() == Some(before) {
            return;
        }
        self.past.push(before.clone());
        if self.past.len() > HISTORY_LIMIT {
            self.past.remove(0);
        }
        self.future.clear();
    }

    pub fn undo(&mut self, current: &Workflow) -> Option<Workflow> {
        let previous = self.past.pop()?;
        self.future.push(current.clone());
        Some(previous)
    }

    pub fn redo(&mut self, current: &Workflow) -> Option<Workflow> {
        let next = self.future.pop()?;
        self.past.push(current.clone());
        Some(next)
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

/// Live state of a node during a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunState {
    Waiting,
    Running,
    Done,
    Failed,
}

pub type States = BTreeMap<String, RunState>;
