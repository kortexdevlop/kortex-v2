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
pub fn Card(children: Element, #[props(default)] class: String) -> Element {
    rsx! {
        div { class: "k-card {class}", {children} }
    }
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
pub fn TextArea(label: String, value: String, oninput: EventHandler<String>) -> Element {
    rsx! {
        label { class: "k-field",
            span { class: "k-muted", "{label}" }
            textarea {
                class: "k-input k-textarea",
                rows: "8",
                spellcheck: "false",
                value: "{value}",
                oninput: move |e| oninput.call(e.value()),
            }
        }
    }
}

/// Native color picker plus the editable `#rrggbb` text.
#[component]
pub fn ColorField(label: String, value: String, oninput: EventHandler<String>) -> Element {
    let picker = if value.len() == 7 && value.starts_with('#') {
        value.clone()
    } else {
        "#000000".to_owned()
    };
    rsx! {
        label { class: "k-field",
            span { class: "k-muted", "{label}" }
            span { class: "k-colorrow",
                input {
                    class: "k-color",
                    r#type: "color",
                    value: "{picker}",
                    oninput: move |e| oninput.call(e.value().to_uppercase()),
                }
                input {
                    class: "k-input",
                    value: "{value}",
                    oninput: move |e| oninput.call(e.value()),
                }
            }
        }
    }
}

#[component]
pub fn Select(
    label: String,
    value: String,
    options: Vec<(String, String)>,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        label { class: "k-field",
            span { class: "k-muted", "{label}" }
            select {
                class: "k-input",
                value: "{value}",
                onchange: move |e| onchange.call(e.value()),
                for (v , text) in options {
                    option { key: "{v}", value: "{v}", selected: v == value, "{text}" }
                }
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
pub fn Progress(value: f32, #[props(default)] indeterminate: bool) -> Element {
    let pct = (value.clamp(0.0, 1.0) * 100.0).round();
    rsx! {
        div { class: "k-progress", role: "progressbar", "aria-valuemin": "0", "aria-valuemax": "100", "aria-valuenow": (!indeterminate).then(|| pct.to_string()),
            div { class: "k-progress-bar", style: "width: {pct}%" }
        }
    }
}

/// Files chosen in a [`DropZone`]. `failed` is set when a file could not be read, so the host
/// can show a translated error instead of failing silently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Picked {
    pub files: Vec<kortex_platform::document::SelectedFile>,
    pub failed: bool,
}

async fn read_all(list: Vec<dioxus::html::FileData>) -> Picked {
    let mut picked = Picked::default();
    for file in &list {
        match kortex_platform::document::read_selected(file).await {
            Ok(f) => picked.files.push(f),
            Err(_) => picked.failed = true,
        }
    }
    picked
}

/// Drop target for files (drag & drop or file picker). Calls `onfiles` with the files read.
/// `accept` lists MIME patterns (`image/*,application/pdf`); `None` accepts every file.
/// Desktop opens the system dialog through `kortex-platform`: the WebView's file input cannot
/// offer every file on macOS.
#[component]
pub fn DropZone(
    label: String,
    hint: String,
    #[props(default)] multiple: bool,
    accept: Option<String>,
    onfiles: EventHandler<Picked>,
) -> Element {
    let mut over = use_signal(|| false);
    let ondrop = move |e: DragEvent| {
        e.prevent_default();
        over.set(false);
        let list = e.data_transfer().files();
        spawn(async move { onfiles.call(read_all(list).await) });
    };
    #[cfg(target_arch = "wasm32")]
    {
        rsx! {
            label {
                class: "k-dropzone",
                "data-over": "{over}",
                ondragover: move |e| {
                    e.prevent_default();
                    over.set(true);
                },
                ondragleave: move |_| over.set(false),
                ondrop,
                input {
                    class: "k-visually-hidden",
                    r#type: "file",
                    multiple,
                    accept,
                    onchange: move |e| {
                        let list = e.files();
                        spawn(async move { onfiles.call(read_all(list).await) });
                    },
                }
                strong { "{label}" }
                span { class: "k-muted", "{hint}" }
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        rsx! {
            button {
                class: "k-dropzone",
                r#type: "button",
                "data-over": "{over}",
                ondragover: move |e| {
                    e.prevent_default();
                    over.set(true);
                },
                ondragleave: move |_| over.set(false),
                ondrop,
                onclick: move |_| {
                    let accept = accept.clone().unwrap_or_default();
                    spawn(async move {
                        match kortex_platform::document::pick_files(&accept, multiple).await {
                            Ok(files) if files.is_empty() => {}
                            Ok(files) => onfiles.call(Picked { files, failed: false }),
                            Err(_) => onfiles.call(Picked { files: Vec::new(), failed: true }),
                        }
                    });
                },
                strong { "{label}" }
                span { class: "k-muted", "{hint}" }
            }
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
