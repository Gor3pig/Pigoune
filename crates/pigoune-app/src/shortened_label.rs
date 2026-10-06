use gtk::pango;

pub const NAME_CHARS: i32 = 24;
pub const PATH_CHARS: i32 = 32;

#[must_use]
pub fn shortened_label(text: &str, max_chars: i32) -> gtk::Label {
    gtk::Label::builder()
        .label(text)
        .ellipsize(pango::EllipsizeMode::End)
        .max_width_chars(max_chars)
        .build()
}

#[must_use]
pub fn name_label(text: &str) -> gtk::Label {
    shortened_label(text, NAME_CHARS)
}

#[must_use]
pub fn naming(name: &str, tooltip: &str) -> String {
    format!("{name}\n{tooltip}")
}
