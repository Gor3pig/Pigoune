const GNOME_ICON_THEME: &str = "Adwaita";

pub fn keep_gnome_icons() {
    let Some(settings) = gtk::Settings::default() else {
        return;
    };
    use_gnome_icons(&settings);
    settings.connect_gtk_icon_theme_name_notify(use_gnome_icons);
}

fn use_gnome_icons(settings: &gtk::Settings) {
    if settings.gtk_icon_theme_name().as_deref() != Some(GNOME_ICON_THEME) {
        settings.set_gtk_icon_theme_name(Some(GNOME_ICON_THEME));
    }
}
