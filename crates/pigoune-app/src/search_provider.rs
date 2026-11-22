use std::collections::HashMap;
use std::path::PathBuf;

use adw::prelude::*;
use gtk::{gdk, gio, glib};
use pigoune_core::{Asset, AssetFilter, AssetId, AssetView, Library, OriginalFile, RgbaImage};

use crate::config::APP_ID;
use crate::settings;
use crate::thumbnails;
use crate::window::PigouneWindow;

const OBJECT_PATH: &str = "/io/github/gor3pig/Pigoune/SearchProvider";
const INTERFACE_NAME: &str = "org.gnome.Shell.SearchProvider2";
const RESULT_LIMIT: usize = 20;
const INACTIVITY_TIMEOUT_MS: u32 = 30_000;
const RESULT_ICON_PIXELS: u32 = 96;
const BITS_PER_SAMPLE: i32 = 8;
const RGBA_CHANNELS: i32 = 4;
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
            let hold = application.hold();
            glib::spawn_future_local(async move {
                let metas = result_metas(&identifiers).await;
                invocation.return_value(Some(&(metas,).to_variant()));
                drop(hold);
            });
        }
        "ActivateResult" => {
            let (identifier, _, _) = parameters
                .get::<(String, Vec<String>, u32)>()
                .unwrap_or_default();
            reveal_in_window(application, AssetId::parse(&identifier).map(Reveal::Asset));
            invocation.return_value(None);
        }
        _ => {
            let (terms, _) = parameters.get::<(Vec<String>, u32)>().unwrap_or_default();
            reveal_in_window(application, Some(Reveal::Search(terms.join(" "))));
            invocation.return_value(None);
        }
    }
}

fn reveal_in_window(application: &adw::Application, reveal: Option<Reveal>) {
    application.activate();
    let window = application.active_window().and_downcast::<PigouneWindow>();
    if let (Some(window), Some(reveal)) = (window, reveal) {
        window.reveal(reveal);
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

#[derive(Debug)]
pub enum Reveal {
    Asset(AssetId),
    Search(String),
}

struct ResultFacts {
    identifier: String,
    name: String,
    original: Option<OriginalFile>,
    thumbnail_file: PathBuf,
}

async fn result_metas(identifiers: &[String]) -> Vec<HashMap<String, glib::Variant>> {
    let mut metas = Vec::new();
    for facts in result_facts(identifiers) {
        metas.push(meta_of(facts).await);
    }
    metas
}

async fn meta_of(facts: ResultFacts) -> HashMap<String, glib::Variant> {
    let mut meta = HashMap::from([
        ("id".to_owned(), facts.identifier.to_variant()),
        ("name".to_owned(), facts.name.to_variant()),
    ]);
    let texture = thumbnails::thumbnail_of(facts.original.as_ref(), &facts.thumbnail_file).await;
    match texture.and_then(|texture| icon_data(&texture)) {
        Some(icon) => meta.insert("icon-data".to_owned(), icon),
        None => meta.insert("gicon".to_owned(), APP_ID.to_variant()),
    };
    meta
}

fn icon_data(texture: &gdk::Texture) -> Option<glib::Variant> {
    let mut downloader = gdk::TextureDownloader::new(texture);
    downloader.set_format(gdk::MemoryFormat::R8g8b8a8);
    let (bytes, stride) = downloader.download_bytes();
    let (width, height) = (texture.width(), texture.height());
    let row_length = usize::try_from(width).ok()? * usize::try_from(RGBA_CHANNELS).ok()?;
    let tight = bytes
        .chunks(stride)
        .flat_map(|row| &row[..row_length])
        .copied()
        .collect();
    let image = RgbaImage::new(
        u32::try_from(width).ok()?,
        u32::try_from(height).ok()?,
        tight,
    )?
    .shrunk_to(RESULT_ICON_PIXELS, RESULT_ICON_PIXELS);
    let (width, height) = (
        i32::try_from(image.width()).ok()?,
        i32::try_from(image.height()).ok()?,
    );
    Some(
        (
            width,
            height,
            width * RGBA_CHANNELS,
            true,
            BITS_PER_SAMPLE,
            RGBA_CHANNELS,
            image.into_pixels(),
        )
            .to_variant(),
    )
}

fn result_facts(identifiers: &[String]) -> Vec<ResultFacts> {
    with_library(|library| {
        identifiers
            .iter()
            .filter_map(|identifier| facts_of(library, identifier))
            .collect()
    })
    .unwrap_or_default()
}

fn facts_of(library: &Library, identifier: &str) -> Option<ResultFacts> {
    let id = AssetId::parse(identifier)?;
    let asset = library.asset(id).ok()??;
    Some(ResultFacts {
        identifier: identifier.to_owned(),
        original: library.original_of(&asset).ok(),
        thumbnail_file: library.thumbnail_file(id, thumbnails::THUMBNAIL_PIXELS),
        name: asset.display_name,
    })
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
