use gettextrs::{gettext, ngettext};
use gtk::glib;
use pigoune_core::{AssetId, AssetView, CollectionId, Library, TagId};

use crate::sidebar::HoveredDrop;

const MAX_NAME_CHARS: usize = 30;

pub fn describe(
    hovered: &HoveredDrop,
    current_view: AssetView,
    library: &Library,
) -> Option<String> {
    if current_view == AssetView::Trash {
        return None;
    }
    let subject = subject(&hovered.assets, library)?;
    let markup = match hovered.view {
        AssetView::Trash => {
            with_subject(&template(&gettext("Move {subject} to the trash")), &subject)
        }
        AssetView::Collection(to) => {
            describe_collection(hovered, current_view, to, &subject, library)?
        }
        AssetView::Tag(tag) => describe_tag(&hovered.assets, tag, &subject, library)?,
        AssetView::Favorites => describe_favorites(&hovered.assets, &subject, library),
        AssetView::Unclassified => describe_unclassified(&hovered.assets, &subject, library),
        AssetView::All => template(&gettext(
            "All already shows every asset, there is nothing to drop here.",
        )),
        AssetView::Smart(_) => template(&gettext(
            "A smart collection fills itself, assets cannot be added to it.",
        )),
    };
    Some(markup)
}

fn describe_collection(
    hovered: &HoveredDrop,
    current_view: AssetView,
    to: CollectionId,
    subject: &str,
    library: &Library,
) -> Option<String> {
    let count = hovered.assets.len();
    let name = shortened(&library.collection(to).ok()??.name);
    if let Some(text) = moving_from_everywhere(hovered, current_view, to, library) {
        return Some(named(&with_subject(&template(&text), subject), &name));
    }
    let text = match current_view {
        AssetView::Collection(from) if from == to => return None,
        AssetView::Collection(_) if !hovered.keep_source => {
            gettext("Move {subject} to the collection {name}")
        }
        _ if all_in_collection(&hovered.assets, to, library) => ngettext(
            "{subject} is already in the collection {name}",
            "{subject} are already in the collection {name}",
            plural(count),
        ),
        _ => gettext("Add {subject} to the collection {name}"),
    };
    Some(named(&with_subject(&template(&text), subject), &name))
}

fn moving_from_everywhere(
    hovered: &HoveredDrop,
    current_view: AssetView,
    to: CollectionId,
    library: &Library,
) -> Option<String> {
    if hovered.keep_source || !matches!(current_view, AssetView::All | AssetView::Unclassified) {
        return None;
    }
    let command = library
        .move_to_collection_command(&hovered.assets, to)
        .ok()??;
    Some(if command.removes_assets() {
        ngettext(
            "Move {subject} to {name} and out of its other collections",
            "Move {subject} to {name} and out of their other collections",
            plural(hovered.assets.len()),
        )
    } else {
        gettext("Move {subject} to the collection {name}")
    })
}

fn describe_tag(
    assets: &[AssetId],
    tag: TagId,
    subject: &str,
    library: &Library,
) -> Option<String> {
    let name = library
        .tags()
        .ok()?
        .into_iter()
        .find(|candidate| candidate.id == tag)?
        .name;
    let text = if all_tagged(assets, tag, library) {
        ngettext(
            "{subject} already has the tag {name}",
            "{subject} already have the tag {name}",
            plural(assets.len()),
        )
    } else {
        gettext("Apply the tag {name} to {subject}")
    };
    Some(named(&with_subject(&template(&text), subject), &name))
}

fn describe_unclassified(assets: &[AssetId], subject: &str, library: &Library) -> String {
    let text = if library
        .unclassify_command(assets)
        .is_ok_and(|command| command.is_some())
    {
        gettext("Remove {subject} from all collections")
    } else {
        ngettext(
            "{subject} is already unclassified",
            "{subject} are already unclassified",
            plural(assets.len()),
        )
    };
    with_subject(&template(&text), subject)
}

fn describe_favorites(assets: &[AssetId], subject: &str, library: &Library) -> String {
    let text = if all_favorite(assets, library) {
        ngettext(
            "{subject} is already in Favorites",
            "{subject} are already in Favorites",
            plural(assets.len()),
        )
    } else {
        ngettext(
            "Add {subject} to Favorites",
            "Add {subject} to Favorites",
            plural(assets.len()),
        )
    };
    with_subject(&template(&text), subject)
}

fn all_favorite(assets: &[AssetId], library: &Library) -> bool {
    library.all_are_favorite(assets).unwrap_or(false)
}

fn subject(assets: &[AssetId], library: &Library) -> Option<String> {
    let name_of = |id: &AssetId| -> Option<String> {
        Some(shortened(&library.asset(*id).ok()??.display_name))
    };
    let first = name_of(assets.first()?)?;
    Some(match assets {
        [_] => bold(&first),
        [_, second] => template(&gettext("{first} and {second}"))
            .replace("{first}", &bold(&first))
            .replace("{second}", &bold(&name_of(second)?)),
        [_, others @ ..] => template(&ngettext(
            "{name} and {count} other",
            "{name} and {count} others",
            plural(others.len()),
        ))
        .replace("{name}", &bold(&first))
        .replace("{count}", &others.len().to_string()),
        [] => return None,
    })
}

fn shortened(name: &str) -> String {
    if name.chars().count() <= MAX_NAME_CHARS {
        return name.to_owned();
    }
    let kept: String = name.chars().take(MAX_NAME_CHARS - 1).collect();
    format!("{}…", kept.trim_end())
}

fn all_in_collection(assets: &[AssetId], collection: CollectionId, library: &Library) -> bool {
    library
        .all_in_collection(assets, collection)
        .unwrap_or(false)
}

fn all_tagged(assets: &[AssetId], tag: TagId, library: &Library) -> bool {
    library.all_have_tag(assets, tag).unwrap_or(false)
}

fn with_subject(text: &str, subject: &str) -> String {
    text.replace("{subject}", subject)
}

fn named(text: &str, name: &str) -> String {
    text.replace("{name}", &bold(name))
}

fn template(text: &str) -> String {
    glib::markup_escape_text(text).to_string()
}

fn bold(name: &str) -> String {
    format!("<b>{}</b>", glib::markup_escape_text(name))
}

fn plural(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use pigoune_core::{
        AssetCommand, AssetFilter, AssetId, AssetView, CollectionCommand, CollectionId,
        ImportOutcome, Library, TagCommand,
    };

    use super::describe;
    use crate::drop_message_reference as reference;
    use crate::sidebar::HoveredDrop;

    const FILES: [&str; 7] = [
        "red-dot.png",
        "dark-circle.svg",
        "blue-photo.jpg",
        "navy-tile.bmp",
        "green-square.webp",
        "blinking.png",
        "blinking.webp",
    ];

    struct Folder(PathBuf);

    impl Folder {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "pigoune-drop-message-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |elapsed| elapsed.as_nanos())
            ));
            std::fs::create_dir_all(&path).expect("temporary folder is created");
            Self(path)
        }
    }

    impl Drop for Folder {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct Dice(u64);

    impl Dice {
        fn below(&mut self, limit: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            usize::try_from(self.0 >> 33).expect("a shifted value fits") % limit.max(1)
        }

        fn some_of<T: Clone>(&mut self, choices: &[T]) -> Vec<T> {
            let count = 1 + self.below(choices.len());
            (0..count)
                .map(|_| choices[self.below(choices.len())].clone())
                .collect()
        }
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../pigoune-core/tests/fixtures")
            .join(name)
    }

    struct World {
        _folder: Folder,
        library: Library,
        assets: Vec<AssetId>,
        collections: Vec<CollectionId>,
    }

    fn world() -> World {
        let folder = Folder::new();
        let mut library = Library::create(&folder.0, "Essai").expect("library is created");
        let mut assets = Vec::new();
        for name in FILES {
            match library
                .import_file(&fixture(name), None)
                .expect("import succeeds")
            {
                ImportOutcome::Imported(id) => assets.push(id),
                outcome => panic!("unexpected outcome {outcome:?}"),
            }
        }
        let top = library.create_collection("Top", None).expect("collection");
        let other = library
            .create_collection("Other", None)
            .expect("collection");
        let nested = library
            .create_collection("Nested", Some(top))
            .expect("collection");
        let deepest = library
            .create_collection("Deepest", Some(nested))
            .expect("collection");
        library
            .create_smart_collection("Photos", &AssetFilter::text("photo"))
            .expect("smart collection");
        World {
            _folder: folder,
            library,
            assets,
            collections: vec![top, other, nested, deepest],
        }
    }

    fn arrange(world: &mut World) {
        let [first, second, third, fourth, ..] = world.collections[..] else {
            panic!("four collections");
        };
        let assets = world.assets.clone();
        for (collection, members) in [
            (first, &assets[..4]),
            (second, &assets[2..5]),
            (third, &assets[..2]),
            (fourth, &assets[..1]),
        ] {
            world
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection,
                    assets: members.to_vec(),
                })
                .expect("assets are filed");
        }
        world
            .library
            .apply_tag_command(&TagCommand::Add {
                assets: assets[1..5].to_vec(),
                name: "Brands/Logo".to_owned(),
            })
            .expect("assets are tagged");
        world
            .library
            .apply_tag_command(&TagCommand::Add {
                assets: assets.clone(),
                name: "Everything".to_owned(),
            })
            .expect("assets are tagged");
        world
            .library
            .apply_asset_command(&AssetCommand::SetFavorite {
                assets: assets[..3].to_vec(),
                favorite: true,
            })
            .expect("favorites are set");
    }

    fn views(world: &World) -> Vec<AssetView> {
        let mut views = vec![
            AssetView::All,
            AssetView::Favorites,
            AssetView::Unclassified,
            AssetView::Trash,
        ];
        views.extend(
            world
                .collections
                .iter()
                .map(|id| AssetView::Collection(*id)),
        );
        views.extend(
            world
                .library
                .tags()
                .expect("tags are listed")
                .into_iter()
                .map(|tag| AssetView::Tag(tag.id)),
        );
        views.extend(
            world
                .library
                .smart_collections()
                .expect("smart collections are listed")
                .into_iter()
                .map(|collection| AssetView::Smart(collection.id)),
        );
        views
    }

    fn selections(world: &World) -> Vec<Vec<AssetId>> {
        let assets = &world.assets;
        let unknown = AssetId::parse("00000000-0000-7000-8000-000000000999").expect("valid id");
        let mut chosen: Vec<Vec<AssetId>> = assets.iter().map(|asset| vec![*asset]).collect();
        chosen.extend([
            assets[..2].to_vec(),
            assets[1..4].to_vec(),
            assets.clone(),
            Vec::new(),
            vec![assets[0], assets[0]],
            vec![assets[1], unknown, assets[2]],
            assets[..4].to_vec(),
            assets[2..5].to_vec(),
            assets[..2].to_vec(),
            assets[..3].to_vec(),
            assets.iter().cycle().take(800).copied().collect(),
        ]);
        chosen
    }

    fn plain(text: &str) -> String {
        let mut result = String::new();
        let mut rest = text;
        while let Some(start) = rest.find("<b>") {
            result.push_str(&rest[..start]);
            result.push('_');
            rest = rest[start..]
                .find("</b>")
                .map_or("", |end| &rest[start + end + 4..]);
        }
        result.push_str(rest);
        result
    }

    fn compare(world: &World, context: &str, seen: &mut std::collections::BTreeSet<String>) {
        let views = views(world);
        let selections = selections(world);
        let mut compared = 0;
        for current in &views {
            for target in &views {
                for keep_source in [false, true] {
                    for assets in &selections {
                        let hovered = HoveredDrop {
                            view: *target,
                            assets: assets.clone(),
                            keep_source,
                        };
                        let now = describe(&hovered, *current, &world.library);
                        let before = reference::describe(&hovered, *current, &world.library);
                        assert_eq!(
                            now,
                            before,
                            "message differs ({context}): from {current:?} to {target:?}, keep {keep_source}, {} assets",
                            assets.len()
                        );
                        compared += 1;
                        if let Some(text) = now {
                            seen.insert(plain(&text));
                        }
                    }
                }
            }
        }
        assert!(compared > 2000, "enough combinations were compared");
    }

    fn gesture(world: &mut World, dice: &mut Dice) {
        let assets = world.assets.clone();
        let collections = world.collections.clone();
        let _ = match dice.below(6) {
            0 => world
                .library
                .apply_collection_command(&CollectionCommand::AddAssets {
                    collection: collections[dice.below(collections.len())],
                    assets: dice.some_of(&assets),
                })
                .map(|_| ())
                .map_err(drop),
            1 => world
                .library
                .apply_collection_command(&CollectionCommand::RemoveAssets {
                    collection: collections[dice.below(collections.len())],
                    assets: dice.some_of(&assets),
                })
                .map(|_| ())
                .map_err(drop),
            2 => world
                .library
                .apply_tag_command(&TagCommand::Add {
                    assets: dice.some_of(&assets),
                    name: "Brands/Logo".to_owned(),
                })
                .map(|_| ())
                .map_err(drop),
            3 => world
                .library
                .apply_asset_command(&AssetCommand::SetFavorite {
                    assets: dice.some_of(&assets),
                    favorite: dice.below(2) == 0,
                })
                .map(|_| ())
                .map_err(drop),
            4 => world
                .library
                .apply_asset_command(&AssetCommand::SetTrashed {
                    assets: dice.some_of(&assets),
                    trashed: dice.below(2) == 0,
                })
                .map(|_| ())
                .map_err(drop),
            _ => world.library.undo().map(|_| ()).map_err(drop),
        };
    }

    #[test]
    fn hover_messages_match_the_per_asset_reference_for_every_destination() {
        let mut world = world();
        arrange(&mut world);
        let mut seen = std::collections::BTreeSet::new();
        compare(&world, "initial state", &mut seen);
        let mut dice = Dice(11);
        for round in 0..4 {
            for _ in 0..3 {
                gesture(&mut world, &mut dice);
            }
            compare(&world, &format!("after round {round}"), &mut seen);
        }
        for expected in [
            "is already in the collection",
            "Move _ to the collection",
            "Add _ to the collection",
            "out of its other collections",
            "already has the tag",
            "Apply the tag",
            "already in Favorites",
            "Add _ to Favorites",
            "Remove _ from all collections",
            "already unclassified",
            "nothing to drop",
            "fills itself",
            "to the trash",
        ] {
            assert!(
                seen.iter().any(|text| text.contains(expected)),
                "no message contained {expected:?}: {seen:?}"
            );
        }
    }
}
