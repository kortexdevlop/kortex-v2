use dioxus::prelude::*;

use crate::Mode;

#[component]
pub fn Button(
    children: Element,
    #[props(default)] ghost: bool,
    #[props(default)] disabled: bool,
    onclick: Option<EventHandler<MouseEvent>>,
) -> Element {
    let variant = if ghost { "ghost" } else { "primary" };
    rsx! {
        button {
            class: "k-btn",
            "data-variant": variant,
            r#type: "button",
            disabled,
            onclick: move |e| {
                if let Some(h) = &onclick {
                    h.call(e);
                }
            },
            {children}
        }
    }
}

#[component]
pub fn Card(children: Element) -> Element {
    rsx! { div { class: "k-card", {children} } }
}

#[component]
pub fn TextField(label: String, value: String, oninput: EventHandler<String>) -> Element {
    rsx! {
        label { class: "k-field",
            span { class: "k-muted", "{label}" }
            input {
                class: "k-input",
                value: "{value}",
                oninput: move |e| oninput.call(e.value()),
            }
        }
    }
}

#[component]
pub fn Switch(label: String, checked: bool, onchange: EventHandler<bool>) -> Element {
    rsx! {
        label { class: "k-switch",
            input {
                r#type: "checkbox",
                role: "switch",
                checked,
                onchange: move |e| onchange.call(e.checked()),
            }
            span { "{label}" }
        }
    }
}

#[component]
pub fn Progress(value: f32) -> Element {
    let pct = (value.clamp(0.0, 1.0) * 100.0).round();
    rsx! {
        div { class: "k-progress", role: "progressbar", "aria-valuemin": "0", "aria-valuemax": "100", "aria-valuenow": "{pct}",
            div { class: "k-progress-bar", style: "width: {pct}%" }
        }
    }
}

/// Drop target for files; the host reads the dropped files from its own handlers.
#[component]
pub fn DropZone(label: String, hint: String) -> Element {
    rsx! {
        div { class: "k-dropzone", tabindex: "0", role: "button", "aria-label": "{label}",
            strong { "{label}" }
            span { class: "k-muted", "{hint}" }
        }
    }
}

#[component]
pub fn ModeSwitch(mode: Mode, onchange: EventHandler<Mode>, labels: [String; 3]) -> Element {
    rsx! {
        div { class: "k-segmented", role: "radiogroup",
            for (m , text) in Mode::ALL.into_iter().zip(labels) {
                button {
                    key: "{m.id()}",
                    class: "k-segment",
                    r#type: "button",
                    role: "radio",
                    "aria-checked": "{m == mode}",
                    onclick: move |_| onchange.call(m),
                    "{text}"
                }
            }
        }
    }
}
