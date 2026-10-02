use std::path::{Path, PathBuf};

use pigoune_core::{
    AssetCommand, AssetId, AssetView, CollectionId, ImportOutcome, Library, TagCommand, TextField,
};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let library = Library::create(workspace.path(), "Essai").expect("library is created");
        Self {
            _workspace: workspace,
            library,
        }
    }

    fn import(&mut self, name: &str, collection: Option<CollectionId>) -> AssetId {
        match self
            .library
            .import_file(&fixture(name), collection)
            .expect("import succeeds")
        {
            ImportOutcome::Imported(id) => id,
            outcome => panic!("unexpected outcome {outcome:?}"),
        }
    }

    fn apply(&mut self, command: &AssetCommand) {
        self.library
            .apply_asset_command(command)
            .expect("command applies");
    }

    fn set_text(&mut self, asset: AssetId, field: TextField, value: &str) {
        self.apply(&AssetCommand::SetText {
            asset,
            field,
            value: value.to_owned(),
        });
    }

    fn tag(&mut self, asset: AssetId, name: &str) {
        self.library
            .apply_tag_command(&TagCommand::Add {
                assets: vec![asset],
                name: name.to_owned(),
            })
            .expect("tag is added");
    }

    fn found(&self, view: AssetView, query: &str) -> Vec<AssetId> {
        let mut ids: Vec<AssetId> = self
            .library
            .search_assets_in(view, query)
            .expect("search succeeds")
            .into_iter()
            .map(|asset| asset.id)
            .collect();
        ids.sort();
        ids
    }
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn sorted(mut ids: Vec<AssetId>) -> Vec<AssetId> {
    ids.sort();
    ids
}

#[test]
fn an_empty_query_shows_the_whole_view() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    let svg = fixture.import("github-mark.svg", None);

    assert_eq!(fixture.found(AssetView::All, "   "), sorted(vec![dot, svg]));
}

#[test]
fn the_name_is_searched_by_any_part_of_a_word() {
    let mut fixture = Fixture::new();
    fixture.import("red-dot.png", None);
    let svg = fixture.import("github-mark.svg", None);

    assert_eq!(fixture.found(AssetView::All, "git"), [svg]);
    assert_eq!(fixture.found(AssetView::All, "MARK"), [svg]);
}

#[test]
fn every_text_field_and_the_tags_are_searched() {
    let mut fixture = Fixture::new();
    let note = fixture.import("red-dot.png", None);
    let source = fixture.import("github-mark.svg", None);
    let license = fixture.import("still.gif", None);
    let author = fixture.import("spinner.gif", None);
    let tagged = fixture.import("green-square.webp", None);
    fixture.set_text(note, TextField::Note, "pour la vitrine");
    fixture.set_text(source, TextField::SourceUrl, "https://exemple.org/logos");
    fixture.set_text(license, TextField::License, "CC-BY-SA");
    fixture.set_text(author, TextField::Author, "Camille Martin");
    fixture.tag(tagged, "printemps");

    assert_eq!(fixture.found(AssetView::All, "vitrine"), [note]);
    assert_eq!(fixture.found(AssetView::All, "exemple.org"), [source]);
    assert_eq!(fixture.found(AssetView::All, "cc-by"), [license]);
    assert_eq!(fixture.found(AssetView::All, "camille"), [author]);
    assert_eq!(fixture.found(AssetView::All, "printemps"), [tagged]);
}

#[test]
fn accents_and_case_do_not_matter() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.set_text(dot, TextField::Note, "Logo de l'École");
    fixture.tag(dot, "Été");

    assert_eq!(fixture.found(AssetView::All, "ecole"), [dot]);
    assert_eq!(fixture.found(AssetView::All, "ÉCOLE"), [dot]);
    assert_eq!(fixture.found(AssetView::All, "ete"), [dot]);
}

#[test]
fn every_word_must_be_found_somewhere() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    let svg = fixture.import("github-mark.svg", None);
    fixture.tag(dot, "rouge");
    fixture.tag(svg, "noir");

    assert_eq!(fixture.found(AssetView::All, "dot rouge"), [dot]);
    assert!(fixture.found(AssetView::All, "dot noir").is_empty());
}

#[test]
fn a_word_cannot_span_two_fields() {
    let mut fixture = Fixture::new();
    let dot = fixture.import("red-dot.png", None);
    fixture.set_text(dot, TextField::Note, "rouge");

    assert!(fixture.found(AssetView::All, "dotrouge").is_empty());
    assert_eq!(fixture.found(AssetView::All, "dot rouge"), [dot]);
}

#[test]
fn the_search_stays_inside_the_selected_view() {
    let mut fixture = Fixture::new();
    let logos = fixture
        .library
        .create_collection("Logos", None)
        .expect("collection is created");
    let inside = fixture.import("red-dot.png", Some(logos));
    let outside = fixture.import("still.gif", None);
    let trashed = fixture.import("spinner.gif", None);
    for asset in [inside, outside, trashed] {
        fixture.tag(asset, "marque");
    }
    fixture.apply(&AssetCommand::SetTrashed {
        assets: vec![trashed],
        trashed: true,
    });

    assert_eq!(
        fixture.found(AssetView::Collection(logos), "marque"),
        [inside]
    );
    assert_eq!(
        fixture.found(AssetView::All, "marque"),
        sorted(vec![inside, outside])
    );
    assert_eq!(fixture.found(AssetView::Trash, "marque"), [trashed]);
}
