use crate::i18n::gettext;
use adw::prelude::*;
use std::{cell::Cell, rc::Rc};

const INSPECTOR_BREAKPOINT: f64 = 1100.0;
const NAVIGATION_BREAKPOINT: f64 = 750.0;

pub(crate) fn build(application: &adw::Application) {
    let navigation_split = adw::OverlaySplitView::builder()
        .sidebar(&build_navigation())
        .min_sidebar_width(190.0)
        .max_sidebar_width(280.0)
        .sidebar_width_fraction(0.22)
        .sidebar_width_unit(adw::LengthUnit::Sp)
        .build();

    let navigation_breakpoint_bin = adw::BreakpointBin::builder()
        .child(&navigation_split)
        .width_request(360)
        .height_request(360)
        .build();

    let inspector_split = adw::OverlaySplitView::builder()
        .content(&navigation_breakpoint_bin)
        .sidebar_position(gtk::PackType::End)
        .min_sidebar_width(260.0)
        .max_sidebar_width(360.0)
        .sidebar_width_fraction(0.27)
        .sidebar_width_unit(adw::LengthUnit::Sp)
        .build();

    inspector_split.set_sidebar(Some(&build_inspector(&inspector_split)));
    navigation_split.set_content(Some(&build_library_view(
        &navigation_split,
        &inspector_split,
    )));

    let window = adw::ApplicationWindow::builder()
        .application(application)
        .title("Pigoune")
        .default_width(1280)
        .default_height(800)
        .content(&inspector_split)
        .build();

    window.add_breakpoint(overlay_breakpoint(&inspector_split, INSPECTOR_BREAKPOINT));
    navigation_breakpoint_bin
        .add_breakpoint(overlay_breakpoint(&navigation_split, NAVIGATION_BREAKPOINT));

    window.present();
}

fn build_navigation() -> adw::ToolbarView {
    let sidebar = adw::Sidebar::new();
    sidebar.set_mode(adw::SidebarMode::Sidebar);

    sidebar.append(sidebar_section(
        &gettext("Library"),
        &[
            (&gettext("All assets"), Some("view-grid-symbolic")),
            (&gettext("Recent"), Some("document-open-recent-symbolic")),
            (&gettext("Favorites"), Some("starred-symbolic")),
            (&gettext("Uncategorized"), Some("folder-symbolic")),
        ],
    ));
    sidebar.append(sidebar_section(
        &gettext("Collections"),
        &[
            (&gettext("Infrastructure"), Some("folder-symbolic")),
            (&gettext("Logos"), Some("folder-symbolic")),
            (&gettext("Systems"), Some("folder-symbolic")),
        ],
    ));
    sidebar.append(sidebar_section(
        &gettext("Tags"),
        &[
            (&gettext("linux"), None),
            (&gettext("network"), None),
            (&gettext("server"), None),
            (&gettext("All tags…"), None),
        ],
    ));
    sidebar.set_selected(0);

    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new("Pigoune", &gettext("Library")))
        .show_end_title_buttons(false)
        .build();

    let view = adw::ToolbarView::builder()
        .top_bar_style(adw::ToolbarStyle::Raised)
        .content(&sidebar)
        .build();
    view.add_top_bar(&header);
    view
}

fn sidebar_section(title: &str, items: &[(&str, Option<&str>)]) -> adw::SidebarSection {
    let section = adw::SidebarSection::new();
    section.set_title(Some(title));

    for (title, icon_name) in items {
        let mut builder = adw::SidebarItem::builder().title(*title);
        if let Some(icon_name) = icon_name {
            builder = builder.icon_name(*icon_name);
        }
        section.append(builder.build());
    }

    section
}

fn build_library_view(
    navigation_split: &adw::OverlaySplitView,
    inspector_split: &adw::OverlaySplitView,
) -> adw::ToolbarView {
    let header = adw::HeaderBar::builder()
        .title_widget(
            &gtk::SearchEntry::builder()
                .placeholder_text(gettext("Search the library…"))
                .hexpand(true)
                .width_chars(22)
                .max_width_chars(38)
                .build(),
        )
        .show_start_title_buttons(false)
        .build();

    header.pack_start(&sidebar_toggle(
        navigation_split,
        "sidebar-show-symbolic",
        &gettext("Show or hide navigation"),
    ));

    let import_button = gtk::Button::builder()
        .label(gettext("Import"))
        .tooltip_text(gettext("Import will be available in a future step"))
        .sensitive(false)
        .build();
    import_button.add_css_class("suggested-action");
    header.pack_end(&import_button);

    header.pack_end(&build_view_switcher());
    header.pack_end(&sidebar_toggle(
        inspector_split,
        "sidebar-show-right-symbolic",
        &gettext("Show or hide inspector"),
    ));

    let empty_state = adw::StatusPage::builder()
        .icon_name("image-x-generic-symbolic")
        .title(gettext("No assets yet"))
        .description(gettext("Imported assets will appear here."))
        .build();

    let view = adw::ToolbarView::builder().content(&empty_state).build();
    view.add_top_bar(&header);
    view
}

fn build_view_switcher() -> gtk::Box {
    let grid_button = gtk::ToggleButton::builder()
        .icon_name("view-grid-symbolic")
        .tooltip_text(gettext("Grid view"))
        .active(true)
        .build();
    let list_button = gtk::ToggleButton::builder()
        .icon_name("view-list-symbolic")
        .tooltip_text(gettext("List view"))
        .group(&grid_button)
        .build();

    let switcher = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    switcher.add_css_class("linked");
    switcher.append(&grid_button);
    switcher.append(&list_button);
    switcher
}

fn build_inspector(inspector_split: &adw::OverlaySplitView) -> adw::ToolbarView {
    let header = adw::HeaderBar::builder()
        .title_widget(&adw::WindowTitle::new(&gettext("Inspector"), ""))
        .show_start_title_buttons(false)
        .build();
    header.pack_end(&sidebar_toggle(
        inspector_split,
        "sidebar-show-right-symbolic",
        &gettext("Hide inspector"),
    ));

    let preview = gtk::Box::builder()
        .width_request(220)
        .height_request(160)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    preview.add_css_class("card");
    preview.append(
        &gtk::Image::builder()
            .icon_name("image-x-generic-symbolic")
            .pixel_size(64)
            .build(),
    );

    let title = gtk::Label::builder()
        .label(gettext("No asset selected"))
        .halign(gtk::Align::Center)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .build();
    title.add_css_class("title-3");

    let details = adw::PreferencesGroup::new();
    details.add(&inspector_section(
        &gettext("Information"),
        &gettext("No information available"),
        true,
    ));
    details.add(&inspector_section(
        &gettext("Organization"),
        &gettext("No collection or tags"),
        false,
    ));
    details.add(&inspector_section(
        &gettext("Technical"),
        &gettext("No technical data"),
        false,
    ));
    details.add(&inspector_section(
        &gettext("Source and licensing"),
        &gettext("No source or licensing information"),
        false,
    ));

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(18)
        .margin_top(18)
        .margin_bottom(18)
        .margin_start(18)
        .margin_end(18)
        .build();
    content.append(&preview);
    content.append(&title);
    content.append(&details);

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&content)
        .build();

    let view = adw::ToolbarView::builder()
        .top_bar_style(adw::ToolbarStyle::Raised)
        .content(&scrolled)
        .build();
    view.add_top_bar(&header);
    view
}

fn inspector_section(title: &str, placeholder: &str, expanded: bool) -> adw::ExpanderRow {
    let section = adw::ExpanderRow::builder()
        .title(title)
        .expanded(expanded)
        .build();
    section.add_row(
        &adw::ActionRow::builder()
            .title(placeholder)
            .sensitive(false)
            .build(),
    );
    section
}

fn sidebar_toggle(
    split_view: &adw::OverlaySplitView,
    icon_name: &str,
    tooltip: &str,
) -> gtk::ToggleButton {
    let button = gtk::ToggleButton::builder()
        .icon_name(icon_name)
        .tooltip_text(tooltip)
        .active(true)
        .build();

    button
        .bind_property("active", split_view, "show-sidebar")
        .bidirectional()
        .sync_create()
        .build();

    button
}

fn overlay_breakpoint(split_view: &adw::OverlaySplitView, max_width: f64) -> adw::Breakpoint {
    let condition = adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        max_width,
        adw::LengthUnit::Sp,
    );
    let breakpoint = adw::Breakpoint::new(condition);
    let was_visible = Rc::new(Cell::new(false));

    let apply_split_view = split_view.clone();
    let apply_was_visible = was_visible.clone();
    breakpoint.connect_apply(move |_| {
        apply_was_visible.set(apply_split_view.shows_sidebar());
        apply_split_view.set_collapsed(true);
    });

    let unapply_split_view = split_view.clone();
    breakpoint.connect_unapply(move |_| {
        unapply_split_view.set_collapsed(false);
        unapply_split_view.set_show_sidebar(was_visible.get());
    });

    breakpoint
}
