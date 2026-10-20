//! Custom UI of {{id}}: rendered by the host (`ToolHost`) on web and desktop from this one source.
use dioxus::prelude::*;
use kortex_ui::{Button, use_tool};

#[component]
pub fn Ui() -> Element {
    let mut tool = use_tool();
    let title = tool.t("ui.title");
    let run_label = tool.t("ui.run");
    rsx! {
        section { class: "k-stack",
            h2 { "{title}" }
            Button {
                disabled: tool.running(),
                onclick: move |_| tool.run(Default::default(), Default::default()),
                "{run_label}"
            }
        }
    }
}
