use gtk::glib;
use pigoune_core::AssetId;

#[derive(Clone, Debug, Default, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedAssets")]
pub struct DraggedAssets(pub Vec<AssetId>);
