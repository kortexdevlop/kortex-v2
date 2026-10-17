use crate::engine::{Dashboard, definitions};
use dioxus::prelude::*;
use kortex_shell::{self as shell, AccountSync, Frame};
use kortex_ui::Button;
use std::sync::OnceLock;
fn locale(lang: &str) -> &'static kortex_i18n::Catalog {
    static FR: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    static EN: OnceLock<kortex_i18n::Catalog> = OnceLock::new();
    if lang == "fr" {
        FR.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/fr.toml")).unwrap_or_default()
        })
    } else {
        EN.get_or_init(|| {
            kortex_i18n::Catalog::from_toml(include_str!("../locales/en.toml")).unwrap_or_default()
        })
    }
}
fn persist(state: Signal<Dashboard>, mut error: Signal<bool>) {
    error.set(!shell::save("hub", "dashboard", &*state.peek()));
}
#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let error = use_signal(|| shell::load::<Dashboard>("hub", "dashboard").1);
    let items = use_hook(shell::available);
    let mut state = use_signal(|| {
        let mut s = shell::load::<Dashboard>("hub", "dashboard").0;
        s.reconcile(&items.iter().map(|i| i.tool.id.clone()).collect::<Vec<_>>());
        s
    });
    let mut query = use_signal(String::new);
    let mut launch_error = use_signal(|| false);
    let mut editing = use_signal(|| false);
    let mut dragged = use_signal(|| None::<String>);
    let widgets = state().widgets;
    let definitions = definitions();
    rsx! {
     Frame {title:l.t("ui.title"),subtitle:l.t("ui.subtitle"),
      style { {include_str!("hub.css")} }
      AccountSync::<Dashboard> {state,kind:kortex_sync::Kind::Hub,
       sanitize:move |mut value:Dashboard|{value.reconcile(&shell::available().iter().map(|e|e.tool.id.clone()).collect::<Vec<_>>());value.launches.clear();value},
       restore:move |mut value:Dashboard|{value.launches=state.peek().launches.clone();state.set(value);persist(state,error);true},
       summary:move |value:Dashboard|format!("{} {} · {} {}",value.favorites.len(),if (appearance.lang)()=="fr"{"favoris"}else{"favourites"},value.widgets.len(),"widgets")
      }
      if launch_error(){p{class:"app-note",role:"alert",{l.t("ui.launch_error")}}}
       if error(){p {class:"app-note",role:"alert",{l.t("ui.storage_error")}}}
      div {class:"hub-toolbar app-actions",
       span {class:"app-badge",{l.t("ui.local")}}
       Button {ghost:true,onclick:move |_|editing.set(!editing()),if editing(){{l.t("ui.done")}}else{{l.t("ui.customize")}}}
       a {class:"k-btn",href:shell::url("desk",&lang),"Kortex Desk →"}
      }
      if editing(){
       div {class:"app-note app-actions",
        for widget in widgets.clone(){
         {let id=widget.id.clone();let label=definitions.iter().find(|d|d.id==id).and_then(|d|d.name.get(&lang)).cloned().unwrap_or(id.clone());rsx!{
          label {class:"hub-widget-toggle",input {r#type:"checkbox",checked:widget.visible,onchange:move |_|{if let Some(w)=state.write().widgets.iter_mut().find(|w|w.id==id){w.visible = !w.visible;}persist(state,error);}}"{label}"}
         }}
        }
        Button{ghost:true,onclick:move |_|{state.write().widgets=Dashboard::default().widgets;persist(state,error);},{l.t("ui.reset")}}
       }
      }
      div {class:"hub-grid",
       for widget in widgets.iter().filter(|w|w.visible).cloned(){
        {let id=widget.id.clone();let heading=definitions.iter().find(|d|d.id==id).and_then(|d|d.name.get(&lang)).cloned().unwrap_or(id.clone());
         let drag_id=id.clone();let drop_id=id.clone();let left=id.clone();let right=id.clone();let wide=id.clone();
         let shown=items.iter().filter(|e|match id.as_str(){"favorites"=>state.read().favorites.contains(&e.tool.id),"recent"=>state.read().recent.contains(&e.tool.id),_=>shell::matches(e,&query(),&lang)}).cloned().collect::<Vec<_>>();
         let shown=if id=="recent"{state.read().recent.iter().filter_map(|id|shown.iter().find(|e|&e.tool.id==id).cloned()).collect()}else{shown};
         rsx!{
          section {key:"{id}",class:"app-panel hub-widget","data-widget":"{id}","data-wide":widget.wide,draggable:editing(),
           ondragstart:move |_|{if editing(){dragged.set(Some(drag_id.clone()));}},ondragover:move |e|e.prevent_default(),
           ondrop:move |e|{e.prevent_default();if let Some(source)=dragged.take(){state.write().reorder(&source,&drop_id);persist(state,error);}},
           div {class:"app-panel-head",h2{"{heading}"}
            if editing(){div {class:"app-actions",
             button {class:"k-btn","data-variant":"ghost",r#type:"button","aria-label":l.t("ui.left"),onclick:move |_|{state.write().move_by(&left,-1);persist(state,error);},"←"}
             button {class:"k-btn","data-variant":"ghost",r#type:"button","aria-label":l.t("ui.right"),onclick:move |_|{state.write().move_by(&right,1);persist(state,error);},"→"}
             button {class:"k-btn","data-variant":"ghost",r#type:"button","aria-label":l.t("ui.wide"),onclick:move |_|{if let Some(w)=state.write().widgets.iter_mut().find(|w|w.id==wide){w.wide = !w.wide;}persist(state,error);},"↔"}
            }}
           }
           if id=="activity"{
            div {class:"hub-metrics",div {p{class:"app-count","{state.read().total()}"}span{class:"k-muted",{l.t("ui.launches")}}}div{p{class:"app-count","{state.read().favorites.len()}"}span{class:"k-muted",{l.t("ui.saved_count")}}}div{p{class:"app-count","{items.len()}"}span{class:"k-muted",{l.t("ui.available")}}}}
            p{class:"k-muted hub-explanation",{l.t("ui.activity_note")}}
           }else{
            if id=="catalogue"{input {class:"k-input app-search",r#type:"search",placeholder:l.t("ui.search"),"aria-label":l.t("ui.search"),value:query(),oninput:move|e|query.set(e.value())}}
            if shown.is_empty(){p {class:"k-muted",if items.is_empty(){{l.t("ui.no_tools")}}else if id=="favorites"{{l.t("ui.no_favorites")}}else if id=="recent"{{l.t("ui.no_recent")}}else{{l.t("ui.empty")}}}}
            ul {class:"app-list hub-tools",
             for entry in shown{
              {let tool=entry.tool.id.clone();let favorite=state.read().favorites.contains(&tool);let favorite_id=tool.clone();let open_id=tool.clone();let label=shell::name(&entry,&lang);rsx!{
               li {key:"{tool}",class:"app-tool","data-tool":"{tool}",div {class:"app-tool-info",strong{"{label}"}small{"{entry.tool.universe}"}}
                button {class:"k-btn","data-variant":"ghost",r#type:"button","aria-label":if favorite{l.t("ui.unfavorite")}else{l.t("ui.favorite")},"aria-pressed":favorite,onclick:move |_|{state.write().favorite(&favorite_id);persist(state,error);},if favorite{"★"}else{"☆"}}
                a {class:"k-btn","data-variant":"ghost",href:format!("{}?tool={tool}",shell::url("desk",&lang)),onclick:move |event|{if !cfg!(target_arch="wasm32"){event.prevent_default();if !shell::launch_installed(&open_id){launch_error.set(true);return;}}launch_error.set(false);state.write().record(&open_id);persist(state,error);},{l.t("ui.open")}}
               }
              }}
             }
            }
           }
          }
         }
        }
       }
      }
      p {class:"app-note hub-account-note",{l.t("ui.account_note")}}
     }
    }
}
