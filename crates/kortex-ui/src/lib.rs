//! Kortex design-system components (Dioxus) and the auto-generated tool UI.
//! Styling comes only from `design/dist/kortex.css` classes (`k-*`) and tokens.
mod auto;
mod components;
mod mode;

pub use auto::{AutoTool, ParamValues, RunFuture, Runner, SaveFile, to_values};
pub use components::{
    Button, Card, ColorField, DropZone, ModeSwitch, Progress, Select, Switch, TextArea, TextField,
};
pub use mode::{Mode, visible_params};
