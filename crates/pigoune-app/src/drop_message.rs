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
        AssetView::All | AssetView::Favorites | AssetView::Unclassified => return None,
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
    let name = library.collection(to).ok()??.name;
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
    assets.iter().all(|asset| {
        library
            .collections_of(*asset)
            .is_ok_and(|collections| collections.contains(&collection))
    })
}

fn all_tagged(assets: &[AssetId], tag: TagId, library: &Library) -> bool {
    assets.iter().all(|asset| {
        library
            .tags_of(*asset)
            .is_ok_and(|tags| tags.iter().any(|carried| carried.id == tag))
    })
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
