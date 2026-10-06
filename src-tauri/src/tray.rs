use std::{path::PathBuf,thread};
use rfd::FileDialog;
use tauri::{image::Image,menu::{CheckMenuItemBuilder,MenuBuilder,MenuItemBuilder,SubmenuBuilder},tray::{MouseButton,MouseButtonState,TrayIconBuilder,TrayIconEvent},AppHandle,Manager};
use crate::{app::AppState,settings::CharacterScale,windows_mgr};
fn snapshot(a:&AppHandle)->(bool,CharacterScale,bool,bool){if let Some(st)=a.try_state::<AppState>(){if let Ok(s)=st.settings.lock(){return(a.get_webview_window("character").is_some(),s.character.scale,s.character.always_on_top,s.performance.low_power);}}(false,CharacterScale::Medium,true,false)}
fn menu(a:&AppHandle)->tauri::Result<tauri::menu::Menu<tauri::Wry>>{
 let(v,scale,top,low)=snapshot(a);
 let show=MenuItemBuilder::with_id("show","Show Saeed").enabled(!v).build(a)?;let hide=MenuItemBuilder::with_id("hide","Hide Saeed").enabled(v).build(a)?;
 let change=MenuItemBuilder::with_id("change","Change Character...").build(a)?;
 let sm=CheckMenuItemBuilder::with_id("small","Small").checked(matches!(scale,CharacterScale::Small)).build(a)?;let md=CheckMenuItemBuilder::with_id("medium","Medium").checked(matches!(scale,CharacterScale::Medium)).build(a)?;let lg=CheckMenuItemBuilder::with_id("large","Large").checked(matches!(scale,CharacterScale::Large)).build(a)?;
 let sizes=SubmenuBuilder::new(a,"Character Size").item(&sm).item(&md).item(&lg).build()?;let at=CheckMenuItemBuilder::with_id("top","Always on Top").checked(top).build(a)?;let lp=CheckMenuItemBuilder::with_id("low","Low Power Mode").checked(low).build(a)?;let rot=MenuItemBuilder::with_id("rotate","Rotate once").build(a)?;let debug=SubmenuBuilder::new(a,"Debug").item(&rot).build()?;let quit=MenuItemBuilder::with_id("quit","Quit").build(a)?;
 MenuBuilder::new(a).item(&show).item(&hide).item(&change).item(&sizes).item(&at).item(&lp).item(&debug).item(&quit).build()
}
pub fn install(a:&AppHandle)->tauri::Result<()>{
 let m=menu(a)?;let icon=Image::from_app_icon_resource(32)?;
 TrayIconBuilder::with_id("default").icon(icon).menu(&m).show_menu_on_left_click(false).on_menu_event(|a,e|match e.id().as_ref(){
 "show"=>{let _=windows_mgr::show_character(a);refresh(a)},
 "hide"=>{let _=crate::app::tray_hide_character(a.clone(),a.state());refresh(a)},
 "change"=>choose(a.clone()),
 "small"=>scale(a,CharacterScale::Small),"medium"=>scale(a,CharacterScale::Medium),"large"=>scale(a,CharacterScale::Large),
 "top"=>{let st=a.state::<AppState>();if let Ok(mut s)=st.settings.lock(){s.character.always_on_top=!s.character.always_on_top;if let Ok(d)=a.path().app_data_dir(){let _=s.save(&d);}if let Some(w)=a.get_webview_window("character"){let _=w.set_always_on_top(s.character.always_on_top);}}refresh(a)},
 "low"=>{let st=a.state::<AppState>();if let Ok(mut s)=st.settings.lock(){s.performance.low_power=!s.performance.low_power;if let Ok(d)=a.path().app_data_dir(){let _=s.save(&d);}}refresh(a)},
 "rotate"=>{let _=crate::app::tray_debug_rotate_once(a.clone())},"quit"=>a.exit(0),_=>{}
 }).on_tray_icon_event(|t,e|{if let TrayIconEvent::Click{button:MouseButton::Left,button_state:MouseButtonState::Up,..}=e{let a=t.app_handle();if a.get_webview_window("character").is_some(){let _=crate::app::tray_hide_character(a.clone(),a.state());}else{let _=windows_mgr::show_character(a);}refresh(&a);}}).build(a)?;
 Ok(())
}
fn choose(a:AppHandle){let _=thread::spawn(move||{if let Some(p)=FileDialog::new().add_filter("GLB model",&["glb"]).pick_file(){let _=crate::app::tray_import_character(a.clone(),p.to_string_lossy().to_string(),a.state());let _=windows_mgr::create_character_window(&a);refresh(&a);}});}
fn scale(a:&AppHandle,s:CharacterScale){let _=crate::app::tray_set_character_scale(a.clone(),s,a.state());if let Some(w)=a.get_webview_window("character"){let n=match s{CharacterScale::Small=>280.0,CharacterScale::Medium=>360.0,CharacterScale::Large=>460.0};let _=w.set_size(tauri::Size::Logical(tauri::LogicalSize{width:n,height:n}));let _=a.emit("character-reload",());}refresh(a);}
pub fn refresh(a:&AppHandle){if let Ok(m)=menu(a){if let Some(t)=a.tray_by_id("default"){let _=t.set_menu(Some(m));}}}