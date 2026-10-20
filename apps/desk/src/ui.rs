use crate::engine::{Layout, Rect, Workspace};
use dioxus::html::InteractionLocation;
use dioxus::prelude::*;
use kortex_shell::{self as shell, AccountSync, Frame, ToolHost};
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
#[derive(Clone, Copy)]
struct Drag {
    id: u64,
    resize: bool,
    start: Rect,
    x: f64,
    y: f64,
}
fn persist(state: Signal<Workspace>, mut error: Signal<bool>) {
    error.set(!shell::save("desk", "workspace", &*state.peek()));
}
#[component]
pub fn App() -> Element {
    let appearance = shell::use_appearance();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    // Recomputed when a newer verified catalogue arrives (installed tools only on desktop).
    let _ = shell::use_catalogue();
    let items = use_memo(|| {
        let _ = shell::catalogue_revision();
        shell::available()
    });
    let items = items();
    let allowed = items.iter().map(|e| e.tool.id.clone()).collect::<Vec<_>>();
    let restored = use_hook(|| {
        let (mut s, mut failed) = shell::load::<Workspace>("desk", "workspace");
        s.reconcile(&allowed);
        if let Some(id) = kortex_platform::document::launch_tool_id()
            && allowed.contains(&id)
            && s.open(&id).is_some()
        {
            failed |= !shell::save("desk", "workspace", &s);
        }
        (s, failed)
    });
    let error = use_signal(|| restored.1);
    let mut state = use_signal(|| restored.0);
    let mut query = use_signal(String::new);
    let mut drag = use_signal(|| None::<Drag>);
    let mut size = use_signal(|| (1000., 650.));
    let mut overview = use_signal(|| false);
    let mut limit = use_signal(|| false);
    let windows = state().windows;
    let active = state().active;
    let mut open = {
        move |id: String| {
            limit.set(state.write().open(&id).is_none());
            persist(state, error);
        }
    };
    rsx! {
     Frame {title:l.t("ui.title"),subtitle:l.t("ui.subtitle"),
      style { {include_str!("desk.css")} }
      AccountSync::<Workspace> {state,kind:kortex_sync::Kind::Desk,can_restore:state.read().windows.is_empty(),
       sanitize:move |mut value:Workspace|{value.reconcile(&shell::available().iter().map(|e|e.tool.id.clone()).collect::<Vec<_>>());value},
       restore:move |value:Workspace|{if !state.peek().windows.is_empty(){return false;}state.set(value);persist(state,error);true},
       summary:move |value:Workspace|format!("{} {} · {} {}",value.spaces.len(),if (appearance.lang)()=="fr"{"espaces"}else{"spaces"},value.windows.len(),if (appearance.lang)()=="fr"{"fenêtres"}else{"windows"})
      }
      if error(){p{class:"app-note",role:"alert",{l.t("ui.storage_error")}}}
      if limit(){p{class:"app-note",role:"alert",{l.t("ui.limit")}}}
      div {class:"desk-toolbar app-actions",
       label{class:"desk-space-label",span{class:"k-muted",{l.t("ui.space")}}
        select{class:"k-input","aria-label":l.t("ui.space"),value:"{active}",onchange:move|e|{if let Ok(id)=e.value().parse::<u64>(){state.write().active=id;persist(state,error);}},
         for space in state().spaces {option{value:"{space.id}","{space.name}"}}
        }
       }
       Button {ghost:true,onclick:move |_|{limit.set(!state.write().add_space());persist(state,error);},"+ " {l.t("ui.new_space")}}
       input {class:"k-input desk-space-name","aria-label":l.t("ui.rename"),maxlength:"40",value:state.read().spaces.iter().find(|s|s.id==active).map(|s|s.name.clone()).unwrap_or_default(),onchange:move|e|{if let Some(space)=state.write().spaces.iter_mut().find(|s|s.id==active){space.name=e.value().chars().take(40).collect();}persist(state,error);}}
       div {class:"desk-arrange app-actions",
        for (key,layout) in [("cascade",Layout::Cascade),("halves",Layout::Halves),("thirds",Layout::Thirds),("quarters",Layout::Quarters),("grid",Layout::Grid)]{
         Button {ghost:true,onclick:move |_|{state.write().arrange(layout);persist(state,error);},{l.t(&format!("ui.{key}")).to_owned()}}
        }
       }
       Button{ghost:true,onclick:move |_|overview.set(!overview()),{l.t("ui.overview")}}
      }
      div {class:"desk-body",
       aside {class:"desk-launcher app-panel","aria-label":l.t("ui.launcher"),h2{{l.t("ui.launcher")}}
        input {class:"k-input app-search",r#type:"search",placeholder:l.t("ui.search"),"aria-label":l.t("ui.search"),value:query(),oninput:move|e|query.set(e.value())}
        if items.is_empty(){p{class:"k-muted",{l.t("ui.no_tools")}}}
        ul {class:"app-list",
         for entry in items.iter().filter(|e|shell::matches(e,&query(),&lang)){
          {let id=entry.tool.id.clone();let name=shell::name(entry,&lang);rsx!{
           li{key:"{id}",button{r#type:"button",class:"desk-launch","data-tool":"{id}",onclick:move |_|open(id.clone()),strong{"{name}"}small{"{entry.tool.universe}"}}}
          }}
         }
        }
       }
       div {
        class:"desk-canvas",id:"desk-canvas","data-dragging":drag().is_some(),"data-overview":overview(),
        onmounted:move|event|{spawn(async move{if let Ok(bounds)=kortex_platform::document::element_size(&event).await{size.set(bounds);}});},
        onpointermove:move|event|{if let Some(d)=drag(){let point=event.client_coordinates();let (width,height)=size();let dx=(point.x-d.x)/width.max(1.)*100.;let dy=(point.y-d.y)/height.max(1.)*100.;let rect=if d.resize{Rect{width:d.start.width+dx,height:d.start.height+dy,..d.start}}else{Rect{x:d.start.x+dx,y:d.start.y+dy,..d.start}};state.write().rect(d.id,rect);}},
        onpointerup:move |_|{if drag.take().is_some(){persist(state,error);}},
        onpointercancel:move |_|{if drag.take().is_some(){persist(state,error);}},
        onpointerleave:move |_|{if drag.take().is_some(){persist(state,error);}},
        if !windows.iter().any(|w|w.space==active&&!w.minimized){div{class:"desk-empty",h2{{l.t("ui.empty_workspace")}}p{class:"k-muted",{l.t("ui.empty_hint")}}}}
        for window in windows.iter(){
         {let id=window.id;let name=shell::find(&window.tool).map(|e|shell::name(&e,&lang)).unwrap_or_else(||window.tool.clone());let rect=window.rect;
          let style=format!("left:calc({}% + var(--k-space-1));top:calc({}% + var(--k-space-1));width:calc({}% - var(--k-space-2));height:calc({}% - var(--k-space-2));z-index:{}",rect.x,rect.y,rect.width,rect.height,window.z);
          rsx!{
           section {key:"{id}",class:"desk-window",style,hidden:window.space!=active||window.minimized,"data-window":"{id}","aria-label":"{name}",
            onpointerdown:move |_|state.write().focus(id),
            div {class:"desk-titlebar",
             button {class:"desk-title",r#type:"button","aria-label":l.t("ui.move_hint"),
              onpointerdown:move|event|{event.prevent_default();let p=event.client_coordinates();state.write().focus(id);drag.set(Some(Drag{id,resize:false,start:rect,x:p.x,y:p.y}));},
              ondoubleclick:move |_|{state.write().maximize(id);persist(state,error);},
              onkeydown:move|event|{let mut r=rect;match event.key().to_string().as_str(){"ArrowLeft"=>r.x-=3.,"ArrowRight"=>r.x+=3.,"ArrowUp"=>r.y-=3.,"ArrowDown"=>r.y+=3.,_=>return}event.prevent_default();state.write().rect(id,r);persist(state,error);},
              "{name}"
             }
             select {class:"desk-transfer","aria-label":l.t("ui.transfer"),value:"{window.space}",onchange:move|event|{if let Ok(space)=event.value().parse::<u64>(){if let Some(w)=state.write().windows.iter_mut().find(|w|w.id==id){w.space=space;}persist(state,error);}},
              for space in state().spaces {option{value:"{space.id}","{space.name}"}}
             }
             button{class:"desk-control",r#type:"button","aria-label":l.t("ui.minimize"),onclick:move |_|{state.write().minimize(id);persist(state,error);},"−"}
             button{class:"desk-control",r#type:"button","aria-label":l.t("ui.maximize"),onclick:move |_|{state.write().maximize(id);persist(state,error);},"□"}
             button{class:"desk-control desk-close",r#type:"button","aria-label":l.t("ui.close"),onclick:move |_|{state.write().close(id);persist(state,error);},"×"}
            }
            div{class:"desk-tool",ToolHost{id:window.tool.clone(),frame:id}}
            button{class:"desk-resize",r#type:"button","aria-label":l.t("ui.resize"),
             onpointerdown:move|event|{event.prevent_default();let p=event.client_coordinates();drag.set(Some(Drag{id,resize:true,start:rect,x:p.x,y:p.y}));},
             onkeydown:move|event|{let mut r=rect;match event.key().to_string().as_str(){"ArrowLeft"=>r.width-=3.,"ArrowRight"=>r.width+=3.,"ArrowUp"=>r.height-=3.,"ArrowDown"=>r.height+=3.,_=>return}event.prevent_default();state.write().rect(id,r);persist(state,error);},"◢"
            }
           }
          }
         }
        }
       }
      }
      nav {class:"desk-dock","aria-label":l.t("ui.dock"),
       for window in windows.iter(){
        {let id=window.id;let title=shell::find(&window.tool).map(|e|shell::name(&e,&lang)).unwrap_or_else(||window.tool.clone());rsx!{button {key:"{id}",class:"k-btn","data-variant":"ghost","data-minimized":window.minimized,r#type:"button",onclick:move |_|{state.write().focus(id);persist(state,error);},"{title}"}}}
       }
      }
      p {class:"desk-footer k-muted",{l.t("ui.restore_note")}}
      p {class:"desk-footer k-muted",{l.t("ui.early")}}
     }
    }
}
