use gettextrs::gettext;
use pigoune_core::annotations::{
    ARROW, BLUR, ELLIPSE, HIGHLIGHTER, LINE, PEN, PIXELATE, RECTANGLE, STEP, TEXT, ToolDefinition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EditorTool {
    #[default]
    Select,
    Crop,
    Rectangle,
    Ellipse,
    Line,
    Arrow,
    Text,
    Steps,
    Pen,
    Highlighter,
    Blur,
    Pixelate,
}

impl EditorTool {
    pub const ALL: [Self; 12] = [
        Self::Select,
        Self::Crop,
        Self::Rectangle,
        Self::Ellipse,
        Self::Line,
        Self::Arrow,
        Self::Text,
        Self::Steps,
        Self::Pen,
        Self::Highlighter,
        Self::Blur,
        Self::Pixelate,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Crop => "crop",
            Self::Rectangle => "rectangle",
            Self::Ellipse => "ellipse",
            Self::Line => "line",
            Self::Arrow => "arrow",
            Self::Text => "text",
            Self::Steps => "steps",
            Self::Pen => "pen",
            Self::Highlighter => "highlighter",
            Self::Blur => "blur",
            Self::Pixelate => "pixelate",
        }
    }

    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Select => "cursor-symbolic",
            Self::Crop => "crop-symbolic",
            Self::Rectangle => "rectangle-symbolic",
            Self::Ellipse => "circle-symbolic",
            Self::Line => "line-segment-symbolic",
            Self::Arrow => "arrow-up-right-symbolic",
            Self::Text => "text-t-symbolic",
            Self::Steps => "number-circle-one-symbolic",
            Self::Pen => "pencil-simple-symbolic",
            Self::Highlighter => "highlighter-symbolic",
            Self::Blur => "drop-symbolic",
            Self::Pixelate => "grid-four-symbolic",
        }
    }

    pub fn label(self) -> String {
        match self {
            Self::Select => gettext("Select"),
            Self::Crop => gettext("Crop"),
            Self::Rectangle => gettext("Rectangle"),
            Self::Ellipse => gettext("Ellipse"),
            Self::Line => gettext("Line"),
            Self::Arrow => gettext("Arrow"),
            Self::Text => gettext("Text"),
            Self::Steps => gettext("Steps"),
            Self::Pen => gettext("Pen"),
            Self::Highlighter => gettext("Highlighter"),
            Self::Blur => gettext("Blur"),
            Self::Pixelate => gettext("Pixelate"),
        }
    }

    pub fn definition(self) -> Option<&'static ToolDefinition> {
        match self {
            Self::Rectangle => Some(&RECTANGLE),
            Self::Ellipse => Some(&ELLIPSE),
            Self::Line => Some(&LINE),
            Self::Arrow => Some(&ARROW),
            Self::Pen => Some(&PEN),
            Self::Highlighter => Some(&HIGHLIGHTER),
            Self::Steps => Some(&STEP),
            Self::Text => Some(&TEXT),
            Self::Blur => Some(&BLUR),
            Self::Pixelate => Some(&PIXELATE),
            _ => None,
        }
    }

    pub fn from_kind(kind: &str) -> Option<Self> {
        match kind {
            "rectangle" => Some(Self::Rectangle),
            "ellipse" => Some(Self::Ellipse),
            "line" => Some(Self::Line),
            "arrow" => Some(Self::Arrow),
            "pen" => Some(Self::Pen),
            "highlighter" => Some(Self::Highlighter),
            "step" => Some(Self::Steps),
            "text" => Some(Self::Text),
            "blur" => Some(Self::Blur),
            "pixelate" => Some(Self::Pixelate),
            _ => None,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tool| tool.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use super::EditorTool;

    #[test]
    fn every_tool_is_found_again_from_its_name() {
        for tool in EditorTool::ALL {
            assert_eq!(EditorTool::from_name(tool.name()), Some(tool));
        }
        assert_eq!(EditorTool::from_name("lasso"), None);
    }

    #[test]
    fn every_registered_annotation_tool_has_a_button_and_a_drawable_one_is_found_again() {
        for definition in pigoune_core::annotations::definitions() {
            let tool = EditorTool::from_kind(definition.kind)
                .unwrap_or_else(|| panic!("{} has no button", definition.kind));
            if let Some(found) = tool.definition() {
                assert_eq!(found.kind, definition.kind);
            }
        }
        assert!(EditorTool::from_kind("lasso").is_none());
    }

    #[test]
    fn tool_names_and_icons_are_all_different() {
        let names: HashSet<_> = EditorTool::ALL.iter().map(|tool| tool.name()).collect();
        let icons: HashSet<_> = EditorTool::ALL
            .iter()
            .map(|tool| tool.icon_name())
            .collect();
        assert_eq!(names.len(), EditorTool::ALL.len());
        assert_eq!(icons.len(), EditorTool::ALL.len());
    }

    #[test]
    fn every_tool_icon_is_shipped_with_the_application() {
        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("icons/scalable/actions");
        let resources = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources.gresource.xml"),
        )
        .expect("resources are listed");
        for tool in EditorTool::ALL {
            let file = format!("{}.svg", tool.icon_name());
            assert!(folder.join(&file).is_file(), "{file} is missing");
            assert!(
                resources.contains(&format!("icons/scalable/actions/{file}")),
                "{file} is not in the resources"
            );
        }
    }
}
