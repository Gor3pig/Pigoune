use std::path::{Path, PathBuf};

use pigoune_core::{AssetCommand, AssetError, AssetId, ImportOutcome, Library, TextField};
use tempfile::TempDir;

struct Fixture {
    _workspace: TempDir,
    library: Library,
    asset: AssetId,
}

impl Fixture {
    fn new() -> Self {
        let workspace = tempfile::tempdir().expect("temporary directory");
        let mut library = Library::create(workspace.path(), "Essai").expect("library is created");
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/github-mark.svg");
        let ImportOutcome::Imported(asset) =
            library.import_file(&source, None).expect("import succeeds")
        else {
            panic!("the fixture is new");
        };
        Self {
            _workspace: workspace,
            library,
            asset,
        }
    }

    fn apply(&mut self, command: &AssetCommand) -> AssetCommand {
        self.library
            .apply_asset_command(command)
            .expect("command applies")
    }

    fn read(&self) -> pigoune_core::Asset {
        self.library
            .asset(self.asset)
            .expect("asset is read")
            .expect("asset exists")
    }

    fn file(&self) -> PathBuf {
        self.library.file_of(&self.read())
    }
}

#[test]
fn renaming_changes_only_the_displayed_name_and_can_be_undone() {
    let mut fixture = Fixture::new();
    let file_before = fixture.file();

    let undo = fixture.apply(&AssetCommand::Rename {
        asset: fixture.asset,
        name: "  Logo GitHub ".to_owned(),
    });
    assert_eq!(fixture.read().display_name, "Logo GitHub");
    assert_eq!(fixture.read().original_file_name, "github-mark.svg");
    assert_eq!(fixture.file(), file_before);

    fixture.apply(&undo);
    assert_eq!(fixture.read().display_name, "github-mark");
}

#[test]
fn a_blank_name_is_refused() {
    let mut fixture = Fixture::new();

    let result = fixture.library.apply_asset_command(&AssetCommand::Rename {
        asset: fixture.asset,
        name: "   ".to_owned(),
    });

    assert!(matches!(result, Err(AssetError::InvalidName)), "{result:?}");
    assert_eq!(fixture.read().display_name, "github-mark");
}

#[test]
fn every_optional_field_starts_empty_and_can_be_set_then_undone() {
    let mut fixture = Fixture::new();
    let values = [
        (TextField::Note, "Logo officiel, version sombre."),
        (TextField::SourceUrl, "https://github.com/logos"),
        (TextField::License, "Usage selon la charte de GitHub"),
        (TextField::Author, "GitHub"),
    ];
    let read_field = |asset: &pigoune_core::Asset, field: TextField| match field {
        TextField::Note => asset.note.clone(),
        TextField::SourceUrl => asset.source_url.clone(),
        TextField::License => asset.license.clone(),
        TextField::Author => asset.author.clone(),
    };

    for (field, value) in values {
        assert_eq!(read_field(&fixture.read(), field), "");
        let undo = fixture.apply(&AssetCommand::SetText {
            asset: fixture.asset,
            field,
            value: format!(" {value} "),
        });
        assert_eq!(read_field(&fixture.read(), field), value);
        fixture.apply(&undo);
        assert_eq!(read_field(&fixture.read(), field), "");
    }
}

#[test]
fn a_field_can_be_emptied_again() {
    let mut fixture = Fixture::new();
    fixture.apply(&AssetCommand::SetText {
        asset: fixture.asset,
        field: TextField::Author,
        value: "GitHub".to_owned(),
    });

    fixture.apply(&AssetCommand::SetText {
        asset: fixture.asset,
        field: TextField::Author,
        value: "  ".to_owned(),
    });

    assert_eq!(fixture.read().author, "");
}
