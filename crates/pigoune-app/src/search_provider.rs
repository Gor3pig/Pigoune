use std::collections::HashMap;
use std::path::PathBuf;

use adw::prelude::*;
use gtk::{gio, glib};
use pigoune_core::{Asset, AssetFilter, AssetView, Library};

use crate::config::APP_ID;
use crate::settings;
use crate::window::PigouneWindow;

const OBJECT_PATH: &str = "/io/github/gor3pig/Pigoune/SearchProvider";
const INTERFACE_NAME: &str = "org.gnome.Shell.SearchProvider2";
const RESULT_LIMIT: usize = 20;
const INACTIVITY_TIMEOUT_MS: u32 = 30_000;
const INTERFACE_XML: &str = r#"
<node>
  <interface name="org.gnome.Shell.SearchProvider2">
    <method name="GetInitialResultSet">
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetSubsearchResultSet">
      <arg type="as" name="previous_results" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="as" name="results" direction="out"/>
    </method>
    <method name="GetResultMetas">
      <arg type="as" name="identifiers" direction="in"/>
      <arg type="aa{sv}" name="metas" direction="out"/>
    </method>
    <method name="ActivateResult">
      <arg type="s" name="identifier" direction="in"/>
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
    <method name="LaunchSearch">
      <arg type="as" name="terms" direction="in"/>
      <arg type="u" name="timestamp" direction="in"/>
    </method>
  </interface>
</node>"#;

pub fn install(application: &adw::Application) {
    application.set_inactivity_timeout(INACTIVITY_TIMEOUT_MS);
    let Some(connection) = application.dbus_connection() else {
        return;
    };
    let Some(interface) = interface_info() else {
        return;
    };
    let application = application.clone();
    let registration = connection
        .register_object(OBJECT_PATH, &interface)
        .method_call(move |_, _, _, _, method, parameters, invocation| {
            let _hold = application.hold();
            answer(&application, method, &parameters, invocation);
        })
        .build();
    if let Err(error) = registration {
        glib::g_warning!(APP_ID, "Search provider not registered: {error}");
    }
}

fn interface_info() -> Option<gio::DBusInterfaceInfo> {
    gio::DBusNodeInfo::for_xml(INTERFACE_XML)
        .ok()?
        .lookup_interface(INTERFACE_NAME)
}

fn answer(
    application: &adw::Application,
    method: &str,
    parameters: &glib::Variant,
    invocation: gio::DBusMethodInvocation,
) {
    match method {
        "GetInitialResultSet" => {
            let (terms,) = parameters.get::<(Vec<String>,)>().unwrap_or_default();
            reply_with_identifiers(invocation, &terms);
        }
        "GetSubsearchResultSet" => {
            let (_, terms) = parameters
                .get::<(Vec<String>, Vec<String>)>()
                .unwrap_or_default();
            reply_with_identifiers(invocation, &terms);
        }
        "GetResultMetas" => {
            let (identifiers,) = parameters.get::<(Vec<String>,)>().unwrap_or_default();
            invocation.return_value(Some(&(result_metas(&identifiers),).to_variant()));
        }
        _ => {
            application.activate();
            invocation.return_value(None);
        }
    }
}

fn reply_with_identifiers(invocation: gio::DBusMethodInvocation, terms: &[String]) {
    let identifiers: Vec<String> = found_assets(terms)
        .iter()
        .take(RESULT_LIMIT)
        .map(|asset| asset.id.to_string())
        .collect();
    invocation.return_value(Some(&(identifiers,).to_variant()));
}

fn result_metas(identifiers: &[String]) -> Vec<HashMap<String, glib::Variant>> {
    let names = names_by_identifier(identifiers);
    identifiers
        .iter()
        .map(|identifier| {
            let name = names.get(identifier).cloned().unwrap_or_default();
            HashMap::from([
                ("id".to_owned(), identifier.to_variant()),
                ("name".to_owned(), name.to_variant()),
                ("gicon".to_owned(), APP_ID.to_variant()),
            ])
        })
        .collect()
}

fn names_by_identifier(identifiers: &[String]) -> HashMap<String, String> {
    with_library(|library| {
        identifiers
            .iter()
            .filter_map(|identifier| {
                let id = pigoune_core::AssetId::parse(identifier)?;
                let asset = library.asset(id).ok()??;
                Some((identifier.clone(), asset.display_name))
            })
            .collect()
    })
    .unwrap_or_default()
}

fn found_assets(terms: &[String]) -> Vec<Asset> {
    let filter = AssetFilter::text(&terms.join(" "));
    with_library(|library| {
        library
            .find_assets_in(AssetView::All, &filter)
            .unwrap_or_default()
    })
    .unwrap_or_default()
}

fn with_library<T>(work: impl FnOnce(&Library) -> T) -> Option<T> {
    let application = gio::Application::default()?;
    if let Some(window) = application
        .downcast_ref::<gtk::Application>()?
        .active_window()
    {
        return window
            .downcast_ref::<PigouneWindow>()?
            .with_open_library(work);
    }
    let path = settings::load().string(settings::LAST_LIBRARY_PATH);
    if path.is_empty() {
        return None;
    }
    let library = Library::open(&PathBuf::from(path.as_str())).ok()?;
    Some(work(&library))
}
