//! Dashboard preferences and recorded launches only; no fabricated processing statistics.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Widget {
    pub id: String,
    pub wide: bool,
    pub visible: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dashboard {
    pub schema: u8,
    pub favorites: BTreeSet<String>,
    pub recent: Vec<String>,
    pub launches: BTreeMap<String, u64>,
    pub widgets: Vec<Widget>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WidgetContract {
    pub schema: u8,
    pub id: String,
    pub name: BTreeMap<String, String>,
}
pub fn definitions() -> Vec<WidgetContract> {
    [
        include_str!("../widgets/favorites/widget.toml"),
        include_str!("../widgets/recent/widget.toml"),
        include_str!("../widgets/catalogue/widget.toml"),
        include_str!("../widgets/activity/widget.toml"),
    ]
    .iter()
    .filter_map(|s| toml::from_str::<WidgetContract>(s).ok())
    .filter(|w| w.schema == 1 && w.name.contains_key("fr") && w.name.contains_key("en"))
    .collect()
}
impl Default for Dashboard {
    fn default() -> Self {
        Self {
            schema: 1,
            favorites: BTreeSet::new(),
            recent: Vec::new(),
            launches: BTreeMap::new(),
            widgets: definitions()
                .iter()
                .map(|w| Widget {
                    id: w.id.clone(),
                    wide: w.id == "catalogue",
                    visible: true,
                })
                .collect(),
        }
    }
}
impl Dashboard {
    pub fn reconcile(&mut self, allowed: &[String]) {
        if self.schema != 1 {
            *self = Self::default();
            return;
        }
        self.favorites.retain(|id| allowed.contains(id));
        let mut seen = BTreeSet::new();
        self.recent
            .retain(|id| allowed.contains(id) && seen.insert(id.clone()));
        self.recent.truncate(12);
        self.launches.retain(|id, _| allowed.contains(id));
        let valid = definitions();
        let mut seen = BTreeSet::new();
        self.widgets
            .retain(|w| valid.iter().any(|v| v.id == w.id) && seen.insert(w.id.clone()));
        for w in valid {
            if !self.widgets.iter().any(|v| v.id == w.id) {
                self.widgets.push(Widget {
                    id: w.id,
                    wide: false,
                    visible: true,
                });
            }
        }
    }
    pub fn favorite(&mut self, id: &str) {
        if !self.favorites.remove(id) {
            self.favorites.insert(id.into());
        }
    }
    pub fn record(&mut self, id: &str) {
        self.recent.retain(|x| x != id);
        self.recent.insert(0, id.into());
        self.recent.truncate(12);
        let count = self.launches.entry(id.into()).or_default();
        *count = count.saturating_add(1);
    }
    pub fn reorder(&mut self, source: &str, target: &str) {
        let Some(from) = self.widgets.iter().position(|w| w.id == source) else {
            return;
        };
        let Some(to) = self.widgets.iter().position(|w| w.id == target) else {
            return;
        };
        let widget = self.widgets.remove(from);
        self.widgets.insert(to, widget);
    }
    pub fn move_by(&mut self, id: &str, delta: isize) {
        if let Some(i) = self.widgets.iter().position(|w| w.id == id) {
            let to = i
                .saturating_add_signed(delta)
                .min(self.widgets.len().saturating_sub(1));
            self.widgets.swap(i, to);
        }
    }
    pub fn total(&self) -> u64 {
        self.launches
            .values()
            .fold(0, |sum, n| sum.saturating_add(*n))
    }
}
