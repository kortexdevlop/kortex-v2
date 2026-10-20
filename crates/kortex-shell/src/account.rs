//! Shared Rust account/backup controls. App models supply typed metadata and guarded restore.
use crate::{Appearance, url};
use dioxus::prelude::*;
use kortex_sync::{Error, Kind, Preferences, Snapshot};
use serde::{Serialize, de::DeserializeOwned};
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
#[derive(Clone)]
struct Pending<T> {
    user: String,
    version: u64,
    snapshot: Snapshot<T>,
    upload: bool,
}
#[derive(Clone, Copy)]
enum Notice {
    Saved,
    Restored,
    Missing,
    CloseWindows,
    Error(Error),
}
impl Notice {
    fn key(self) -> &'static str {
        match self {
            Self::Saved => "account.saved",
            Self::Restored => "account.restored",
            Self::Missing => "account.missing",
            Self::CloseWindows => "account.close_windows",
            Self::Error(e) => match e {
                Error::SignedOut | Error::SessionChanged => "account.expired",
                Error::InvalidResponse => "account.invalid",
                Error::RateLimited => "account.rate_limited",
                Error::Conflict => "account.conflict",
                Error::TooLarge => "account.too_large",
                Error::Unsupported => "account.native",
                Error::Network => "account.network",
            },
        }
    }
}
#[component]
pub fn AccountSync<T: Serialize + DeserializeOwned + Clone + PartialEq + 'static>(
    state: Signal<T>,
    kind: Kind,
    sanitize: Callback<T, T>,
    restore: Callback<T, bool>,
    summary: Callback<T, String>,
    #[props(default = true)] can_restore: bool,
) -> Element {
    let mut appearance = use_context::<Appearance>();
    let lang = (appearance.lang)();
    let l = locale(&lang);
    let mut reload = use_signal(|| 0_u64);
    let session = use_resource(move || async move {
        let _ = reload();
        kortex_auth::session().await
    });
    let result = session.value().read().clone();
    let mut busy = use_signal(|| false);
    let mut notice = use_signal(|| None::<Notice>);
    let mut pending = use_signal(|| None::<Pending<T>>);
    use_effect(move || {
        let value = session.value();
        let user = value
            .read()
            .as_ref()
            .and_then(|r| r.as_ref().ok())
            .and_then(|u| u.as_ref())
            .map(|u| u.id.clone());
        if pending
            .peek()
            .as_ref()
            .is_some_and(|p| Some(&p.user) != user.as_ref())
        {
            pending.set(None);
        }
    });
    let profile = result
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|u| u.as_ref())
        .cloned();
    let account_url = format!(
        "{}{}",
        url("root", &lang),
        if lang == "fr" { "compte" } else { "account" }
    );
    let pending_value = pending();
    rsx! {
     details{class:"app-account app-panel", "data-account":true,
      summary{class:"app-account-title",{l.t("account.title")}}
      div{class:"app-stack app-account-content",
       if !kortex_auth::supported(){p{class:"k-muted",{l.t("account.native")}}}
       else if result.is_none(){p{role:"status",{l.t("account.loading")}}}
       else if let Some(user)=profile.clone(){
        p{class:"app-account-email","{user.email}"}
        p{class:"k-muted",if kind==Kind::Hub{{l.t("account.consent_hub")}}else{{l.t("account.consent_desk")}}}
        p{class:"k-muted",{l.t("account.manual")}}
        div{class:"app-actions",
         button{class:"k-btn",r#type:"button",disabled:busy(),"data-backup-upload":true,
          onclick:move |_|{
           if busy(){return;}
           let local=Snapshot{schema:1,kind,data:sanitize.call(state.peek().clone()),preferences:Preferences{lang:(appearance.lang)(),theme:(appearance.theme)(),mode:(appearance.mode)()}};
           if let Err(e)=local.encode(){notice.set(Some(Notice::Error(e)));return;}
           let user=user.id.clone();busy.set(true);notice.set(None);pending.set(None);
           spawn(async move{
            match kortex_sync::read::<T>(&user,kind).await{
             Ok(Some(remote))=>pending.set(Some(Pending{user,version:remote.version,snapshot:local,upload:true})),
             Ok(None)=>match kortex_sync::write(&user,0,&local).await{Ok(_)=>notice.set(Some(Notice::Saved)),Err(e)=>notice.set(Some(Notice::Error(e)))},
             Err(e)=>notice.set(Some(Notice::Error(e)))
            }busy.set(false);
           });
          },{l.t("account.upload")}
         }
         {let user=profile.as_ref().map(|u|u.id.clone()).unwrap_or_default();rsx!{
          button{class:"k-btn","data-variant":"ghost",r#type:"button",disabled:busy(),"data-backup-download":true,
           onclick:move |_|{if busy(){return;}let user=user.clone();busy.set(true);notice.set(None);pending.set(None);spawn(async move{
            match kortex_sync::read::<T>(&user,kind).await{
             Ok(Some(remote))=>pending.set(Some(Pending{user,version:remote.version,snapshot:remote.snapshot,upload:false})),
             Ok(None)=>notice.set(Some(Notice::Missing)),Err(e)=>notice.set(Some(Notice::Error(e)))
            }busy.set(false);
           });},{l.t("account.download")}
          }
         }}
        }
       }else if let Some(Err(error))=result{p{class:"k-muted",role:"alert",{l.t(Notice::Error(error).key())}}}
       else{p{class:"k-muted",{l.t("account.signed_out")}}}
       if busy(){p{role:"status",{l.t("account.busy")}}}
       if let Some(message)=notice(){p{role:if matches!(message,Notice::Error(_)|Notice::CloseWindows){"alert"}else{"status"},{l.t(message.key())}}}
       if let Some(value)=pending_value{
        div{class:"app-note", "data-backup-preview":true,
         p{{l.t(if value.upload{"account.replace"}else{"account.restore"})}}
         if !value.upload && !can_restore {p{role:"alert",{l.t("account.close_windows")}}}
      p{class:"k-muted",{format!("{} {} · {}",l.t("account.version"),value.version,summary.call(value.snapshot.data.clone()))}}
         div{class:"app-actions",
          button{class:"k-btn",r#type:"button",disabled:busy() || (!value.upload && !can_restore),"data-backup-confirm":true,
           onclick:move |_|{if busy(){return;}let value=value.clone();busy.set(true);notice.set(None);spawn(async move{
            let result=if value.upload{kortex_sync::write(&value.user,value.version,&value.snapshot).await.map(|_|Notice::Saved)}else{
             match kortex_auth::verify_user(&value.user).await{Err(e)=>Err(e),Ok(())=>{
              if restore.call(sanitize.call(value.snapshot.data)){
               appearance.lang.set(value.snapshot.preferences.lang);appearance.theme.set(value.snapshot.preferences.theme);appearance.mode.set(value.snapshot.preferences.mode);Ok(Notice::Restored)
              }else{Ok(Notice::CloseWindows)}
             }}
            };
            notice.set(Some(match result{Ok(n)=>n,Err(e)=>Notice::Error(e)}));pending.set(None);busy.set(false);
           });},if value.upload{{l.t("account.confirm_upload")}}else{{l.t("account.confirm_restore")}}
          }
          button{class:"k-btn","data-variant":"ghost",r#type:"button",disabled:busy(),onclick:move |_|pending.set(None),{l.t("account.cancel")}}
         }
        }
       }
       div{class:"app-actions",
        a{class:"k-btn","data-variant":"ghost",href:account_url,target:"_blank",rel:"noopener noreferrer",if profile.is_some(){{l.t("account.manage")}}else{{l.t("account.connect")}}}
        if kortex_auth::supported(){button{class:"k-btn","data-variant":"ghost",r#type:"button",disabled:busy(),onclick:move |_|{pending.set(None);notice.set(None);let next=reload.peek().saturating_add(1);reload.set(next);},{l.t("account.refresh")}}}
       }
      }
     }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_locales_match() {
        assert!(locale("fr").missing_in(locale("en")).is_empty());
        assert!(locale("en").missing_in(locale("fr")).is_empty());
    }
}
