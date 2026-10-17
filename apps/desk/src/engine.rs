//! Pure workspace model. Only layout metadata is persisted, never files or engine results.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Default for Rect {
    fn default() -> Self {
        Self {
            x: 4.,
            y: 4.,
            width: 64.,
            height: 72.,
        }
    }
}
impl Rect {
    pub fn bounded(self) -> Self {
        let finite = |x: f64, fallback: f64| if x.is_finite() { x } else { fallback };
        let width = finite(self.width, 64.).clamp(24., 100.);
        let height = finite(self.height, 72.).clamp(16., 100.);
        Self {
            x: finite(self.x, 0.).clamp(0., 100. - width),
            y: finite(self.y, 0.).clamp(0., 100. - height),
            width,
            height,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    #[serde(default)]
    pub z: u64,
    pub id: u64,
    pub tool: String,
    pub space: u64,
    pub rect: Rect,
    pub minimized: bool,
    pub restore: Option<Rect>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Space {
    pub id: u64,
    pub name: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub schema: u8,
    pub windows: Vec<Window>,
    pub spaces: Vec<Space>,
    pub active: u64,
    pub next: u64,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            schema: 1,
            windows: Vec::new(),
            spaces: vec![Space {
                id: 1,
                name: "1".into(),
            }],
            active: 1,
            next: 1,
        }
    }
}
#[derive(Clone, Copy)]
pub enum Layout {
    Cascade,
    Halves,
    Thirds,
    Quarters,
    Grid,
}
impl Workspace {
    pub fn reconcile(&mut self, allowed: &[String]) {
        if self.schema != 1 || self.spaces.is_empty() {
            *self = Self::default();
            return;
        }
        self.spaces.truncate(6);
        let mut seen = std::collections::BTreeSet::new();
        self.spaces
            .retain(|s| s.id > 0 && s.id < 9_007_199_254_740_991 && seen.insert(s.id));
        for space in &mut self.spaces {
            space.name = space.name.chars().take(40).collect();
        }
        if self.spaces.is_empty() {
            *self = Self::default();
            return;
        }
        if !self.spaces.iter().any(|s| s.id == self.active) {
            self.active = self.spaces[0].id;
        }
        let mut ids = std::collections::BTreeSet::new();
        self.windows.retain(|w| {
            w.id > 0
                && w.id < 9_007_199_254_740_991
                && ids.insert(w.id)
                && allowed.contains(&w.tool)
                && self.spaces.iter().any(|s| s.id == w.space)
        });
        self.windows.truncate(12);
        for w in &mut self.windows {
            w.rect = w.rect.bounded();
            w.restore = w.restore.map(Rect::bounded);
        }
        let mut layers = self.windows.iter().map(|w| (w.z, w.id)).collect::<Vec<_>>();
        layers.sort();
        for window in &mut self.windows {
            window.z = layers
                .iter()
                .position(|(_, id)| *id == window.id)
                .map_or(1, |i| i as u64 + 1);
        }
        self.next = self
            .windows
            .iter()
            .map(|w| w.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
    }
    pub fn open(&mut self, tool: &str) -> Option<u64> {
        if self.windows.len() >= 12 || self.next == u64::MAX {
            return None;
        }
        let id = self.next;
        self.next += 1;
        let n = self
            .windows
            .iter()
            .filter(|w| w.space == self.active)
            .count() as f64;
        self.windows.push(Window {
            z: self
                .windows
                .iter()
                .map(|w| w.z)
                .max()
                .unwrap_or(0)
                .saturating_add(1),
            id,
            tool: tool.into(),
            space: self.active,
            rect: Rect {
                x: 4. + n * 3.,
                y: 4. + n * 3.,
                ..Rect::default()
            }
            .bounded(),
            minimized: false,
            restore: None,
        });
        Some(id)
    }
    pub fn focus(&mut self, id: u64) {
        let z = self
            .windows
            .iter()
            .map(|w| w.z)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = false;
            self.active = w.space;
            w.z = z;
        }
    }
    pub fn close(&mut self, id: u64) {
        self.windows.retain(|w| w.id != id);
    }
    pub fn minimize(&mut self, id: u64) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.minimized = true;
        }
    }
    pub fn maximize(&mut self, id: u64) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            if let Some(r) = w.restore.take() {
                w.rect = r;
            } else {
                w.restore = Some(w.rect);
                w.rect = Rect {
                    x: 0.,
                    y: 0.,
                    width: 100.,
                    height: 100.,
                };
            }
        }
    }
    pub fn rect(&mut self, id: u64, rect: Rect) {
        if let Some(w) = self.windows.iter_mut().find(|w| w.id == id) {
            w.rect = rect.bounded();
            w.restore = None;
        }
    }
    pub fn add_space(&mut self) -> bool {
        if self.spaces.len() >= 6 {
            return false;
        }
        let id = self
            .spaces
            .iter()
            .map(|s| s.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        self.spaces.push(Space {
            id,
            name: id.to_string(),
        });
        self.active = id;
        true
    }
    pub fn arrange(&mut self, layout: Layout) {
        let count = self
            .windows
            .iter()
            .filter(|w| w.space == self.active && !w.minimized)
            .count();
        let columns = match layout {
            Layout::Cascade => 0,
            Layout::Halves => 2,
            Layout::Thirds => 3,
            Layout::Quarters => 2,
            Layout::Grid => (count as f64).sqrt().ceil() as usize,
        }
        .max(1);
        let rows = count
            .div_ceil(columns)
            .max(if matches!(layout, Layout::Quarters) {
                2
            } else {
                1
            });
        for (i, w) in self
            .windows
            .iter_mut()
            .filter(|w| w.space == self.active && !w.minimized)
            .enumerate()
        {
            w.restore = None;
            w.rect = if matches!(layout, Layout::Cascade) {
                Rect {
                    x: 3. + (i % 6) as f64 * 4.,
                    y: 3. + (i % 6) as f64 * 4.,
                    width: 64.,
                    height: 70.,
                }
            } else {
                Rect {
                    x: (i % columns) as f64 * 100. / columns as f64,
                    y: (i / columns) as f64 * 100. / rows as f64,
                    width: 100. / columns as f64,
                    height: 100. / rows as f64,
                }
            }
            .bounded();
        }
    }
}
