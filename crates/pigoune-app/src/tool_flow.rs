use crate::editor_tool::EditorTool;

#[must_use]
pub fn tool_after_creation(active: EditorTool, persistent: Option<EditorTool>) -> EditorTool {
    if active.creates() && persistent != Some(active) {
        EditorTool::Select
    } else {
        active
    }
}

#[must_use]
pub fn persistence_after_toggle(
    persistent: Option<EditorTool>,
    tool: EditorTool,
) -> Option<EditorTool> {
    if !tool.creates() || persistent == Some(tool) {
        None
    } else {
        Some(tool)
    }
}

#[must_use]
pub fn persistence_after_switch(
    persistent: Option<EditorTool>,
    active: EditorTool,
) -> Option<EditorTool> {
    persistent.filter(|tool| *tool == active)
}

#[must_use]
pub fn leaves_persistence_on_escape(persistent: Option<EditorTool>) -> bool {
    persistent.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CREATING: [EditorTool; 11] = [
        EditorTool::Rectangle,
        EditorTool::Ellipse,
        EditorTool::Line,
        EditorTool::Arrow,
        EditorTool::Text,
        EditorTool::Steps,
        EditorTool::Pen,
        EditorTool::Highlighter,
        EditorTool::Blur,
        EditorTool::Pixelate,
        EditorTool::Spotlight,
    ];

    #[test]
    fn every_creating_tool_returns_to_select_after_one_creation() {
        for tool in CREATING {
            assert!(tool.creates());
            assert_eq!(tool_after_creation(tool, None), EditorTool::Select);
        }
    }

    #[test]
    fn the_persistent_tool_stays_active_after_a_creation() {
        for tool in CREATING {
            assert_eq!(tool_after_creation(tool, Some(tool)), tool);
        }
    }

    #[test]
    fn another_tool_being_persistent_does_not_keep_this_one_active() {
        assert_eq!(
            tool_after_creation(EditorTool::Arrow, Some(EditorTool::Rectangle)),
            EditorTool::Select
        );
    }

    #[test]
    fn select_and_crop_never_become_persistent_or_switch_on_creation() {
        for tool in [EditorTool::Select, EditorTool::Crop] {
            assert!(!tool.creates());
            assert_eq!(persistence_after_toggle(None, tool), None);
            assert_eq!(tool_after_creation(tool, None), tool);
        }
    }

    #[test]
    fn toggling_twice_leaves_the_persistent_mode() {
        let on = persistence_after_toggle(None, EditorTool::Ellipse);
        assert_eq!(on, Some(EditorTool::Ellipse));
        assert_eq!(persistence_after_toggle(on, EditorTool::Ellipse), None);
    }

    #[test]
    fn toggling_another_tool_moves_the_persistent_mode_to_it() {
        let on = persistence_after_toggle(None, EditorTool::Ellipse);
        assert_eq!(
            persistence_after_toggle(on, EditorTool::Line),
            Some(EditorTool::Line)
        );
    }

    #[test]
    fn switching_to_any_other_tool_ends_the_persistent_mode() {
        let on = Some(EditorTool::Rectangle);
        assert_eq!(persistence_after_switch(on, EditorTool::Rectangle), on);
        for other in [EditorTool::Select, EditorTool::Crop, EditorTool::Arrow] {
            assert_eq!(persistence_after_switch(on, other), None);
        }
        assert_eq!(persistence_after_switch(None, EditorTool::Arrow), None);
    }

    #[test]
    fn escape_leaves_the_persistent_mode_only_when_there_is_one() {
        assert!(leaves_persistence_on_escape(Some(EditorTool::Pen)));
        assert!(!leaves_persistence_on_escape(None));
    }
}
