use gtk::glib;
use pigoune_core::{AssetId, CollectionId, SmartCollectionId};

#[derive(Clone, Debug, Default, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedAssets")]
pub struct DraggedAssets(pub Vec<AssetId>);

#[derive(Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedCollection")]
pub struct DraggedCollection(pub CollectionId);

#[derive(Clone, Debug, PartialEq, Eq, glib::Boxed)]
#[boxed_type(name = "PigouneDraggedSmartCollection")]
pub struct DraggedSmartCollection(pub SmartCollectionId);
