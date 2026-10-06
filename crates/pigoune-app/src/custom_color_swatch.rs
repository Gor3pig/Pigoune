use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gettextrs::gettext;
use gtk::prelude::*;
use gtk::{gdk, glib};
use pigoune_core::Rgb;

const CHECKMARK_ICON: &str = "object-select-symbolic";
const LIGHT_INK: &str = "#ffffff";
const DARK_INK: &str = "#1e1e1e";
const DARK_INK_ABOVE_LUMINANCE: f64 = 160.0;
const COLOR_CHOOSER_DIALOG: &str = "GtkColorChooserDialog";
const COLOR_CHOOSER_WIDGET: &str = "GtkColorChooserWidget";
const EDITOR_PROPERTY: &str = "show-editor";

type ChangedCallback = Box<dyn Fn()>;

thread_local! {
    static NEXT_SWATCH: Cell<u32> = const { Cell::new(0) };
}

pub struct CustomColorSwatch {
    button: gtk::ToggleButton,
    provider: gtk::CssProvider,
    class: String,
    color: Cell<Option<Rgb>>,
    quiet: Cell<bool>,
    on_changed: RefCell<Option<ChangedCallback>>,
}

impl CustomColorSwatch {
    pub fn new() -> Rc<Self> {
        let class = NEXT_SWATCH.with(|next| {
            let number = next.get();
            next.set(number + 1);
            format!("custom-swatch-{number}")
        });
        let button = gtk::ToggleButton::builder()
            .child(&gtk::Image::from_icon_name(CHECKMARK_ICON))
            .halign(gtk::Align::Center)
            .css_classes([
                "collection-swatch",
                "resource-swatch",
                "resource-swatch-custom",
                class.as_str(),
            ])
            .build();
        let provider = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let swatch = Rc::new(Self {
            button,
            provider,
            class,
            color: Cell::new(None),
            quiet: Cell::new(false),
            on_changed: RefCell::new(None),
        });
        let weak = Rc::downgrade(&swatch);
        swatch.button.connect_toggled(move |_| {
            if let Some(swatch) = weak.upgrade() {
                swatch.toggled();
            }
        });
        swatch.show();
        swatch
    }

    pub fn button(&self) -> &gtk::ToggleButton {
        &self.button
    }

    pub fn color(&self) -> Option<Rgb> {
        self.color.get()
    }

    pub fn set_color(&self, color: Option<Rgb>) {
        self.color.set(color);
        self.quietly(|| self.button.set_active(color.is_some()));
        self.show();
    }

    pub fn connect_changed(&self, callback: impl Fn() + 'static) {
        self.on_changed.replace(Some(Box::new(callback)));
    }

    fn toggled(self: &Rc<Self>) {
        if self.quiet.get() {
            return;
        }
        if self.button.is_active() {
            self.quietly(|| self.button.set_active(false));
            glib::spawn_future_local(choose_color(Rc::downgrade(self)));
        } else {
            self.color.set(None);
            self.show();
            self.changed();
        }
    }

    fn chosen(&self, color: Rgb) {
        self.set_color(Some(color));
        self.changed();
    }

    fn show(&self) {
        if let Some(color) = self.color.get() {
            self.show_chosen(color);
        } else {
            self.provider.load_from_string("");
            self.describe(
                &gettext("Choose a Custom Color…"),
                &gettext("Choose a Custom Color…"),
            );
        }
    }

    fn show_chosen(&self, color: Rgb) {
        self.provider.load_from_string(&format!(
            ".collection-swatch.resource-swatch-custom.{} {{ background-image: none; background-color: #{}; color: {}; }}",
            self.class,
            color.hex(),
            ink_on(color),
        ));
        let code = code_of(color);
        self.describe(
            &gettext("Custom color {code}, click to remove it").replace("{code}", &code),
            &gettext("Custom Color {code}").replace("{code}", &code),
        );
    }

    fn describe(&self, tooltip: &str, name: &str) {
        self.button.set_tooltip_text(Some(tooltip));
        self.button
            .update_property(&[gtk::accessible::Property::Label(name)]);
    }

    fn quietly(&self, change: impl FnOnce()) {
        self.quiet.set(true);
        change();
        self.quiet.set(false);
    }

    fn changed(&self) {
        if let Some(callback) = self.on_changed.borrow().as_ref() {
            callback();
        }
    }
}

impl Drop for CustomColorSwatch {
    fn drop(&mut self) {
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_remove_provider_for_display(&display, &self.provider);
        }
    }
}

async fn choose_color(swatch: Weak<CustomColorSwatch>) {
    let Some(button) = swatch.upgrade().map(|swatch| swatch.button.clone()) else {
        return;
    };
    let dialog = gtk::ColorDialog::builder()
        .title(gettext("Custom Color"))
        .with_alpha(false)
        .modal(true)
        .build();
    let parent = button.root().and_downcast::<gtk::Window>();
    if let Some(menu) = button
        .ancestor(gtk::Popover::static_type())
        .and_downcast::<gtk::Popover>()
    {
        menu.popdown();
    }
    let choice = dialog.choose_rgba_future(parent.as_ref(), None);
    glib::idle_add_local_once(move || fit_color_chooser_to_its_views(parent.as_ref()));
    let Ok(rgba) = choice.await else {
        return;
    };
    if let Some(swatch) = swatch.upgrade() {
        swatch.chosen(rgb_of(&rgba));
    }
}

fn fit_color_chooser_to_its_views(parent: Option<&gtk::Window>) {
    let choosers = gtk::Window::list_toplevels()
        .into_iter()
        .filter_map(|toplevel| toplevel.downcast::<gtk::Window>().ok())
        .filter(|window| window.type_().name() == COLOR_CHOOSER_DIALOG)
        .filter(|window| window.transient_for().as_ref() == parent);
    for window in choosers {
        let Some(chooser) = descendant_of_type(window.upcast_ref(), COLOR_CHOOSER_WIDGET) else {
            continue;
        };
        if let Some(scrolled) = chooser
            .ancestor(gtk::ScrolledWindow::static_type())
            .and_downcast::<gtk::ScrolledWindow>()
        {
            scrolled.set_max_content_height(-1);
            scrolled.set_propagate_natural_height(true);
        }
        chooser.connect_notify_local(
            Some(EDITOR_PROPERTY),
            glib::clone!(
                #[weak]
                window,
                move |_, _| {
                    glib::idle_add_local_once(glib::clone!(
                        #[weak]
                        window,
                        move || fit_to_content(&window)
                    ));
                }
            ),
        );
    }
}

fn descendant_of_type(root: &gtk::Widget, type_name: &str) -> Option<gtk::Widget> {
    let mut waiting = vec![root.clone()];
    while let Some(widget) = waiting.pop() {
        if widget.type_().name() == type_name {
            return Some(widget);
        }
        let mut child = widget.first_child();
        while let Some(current) = child {
            child = current.next_sibling();
            waiting.push(current);
        }
    }
    None
}

fn fit_to_content(window: &gtk::Window) {
    window.set_size_request(-1, -1);
    let (_, width, _, _) = window.measure(gtk::Orientation::Horizontal, -1);
    let (_, height, _, _) = window.measure(gtk::Orientation::Vertical, width);
    window.set_size_request(width, height);
}

#[must_use]
pub fn code_of(color: Rgb) -> String {
    format!("#{}", color.hex().to_uppercase())
}

fn rgb_of(rgba: &gdk::RGBA) -> Rgb {
    let channel = |value: f32| {
        (0..=u8::MAX)
            .min_by(|first, second| {
                let gap = |byte: &u8| (f32::from(*byte) / 255.0 - value).abs();
                gap(first).total_cmp(&gap(second))
            })
            .unwrap_or_default()
    };
    Rgb::new(
        channel(rgba.red()),
        channel(rgba.green()),
        channel(rgba.blue()),
    )
}

pub fn ink_on(color: Rgb) -> &'static str {
    let luminance = 0.2126 * f64::from(color.red)
        + 0.7152 * f64::from(color.green)
        + 0.0722 * f64::from(color.blue);
    if luminance > DARK_INK_ABOVE_LUMINANCE {
        DARK_INK
    } else {
        LIGHT_INK
    }
}

#[cfg(test)]
mod tests {
    use pigoune_core::Rgb;

    use super::{DARK_INK, LIGHT_INK, code_of, ink_on, rgb_of};

    #[test]
    fn a_chosen_color_keeps_its_exact_bytes() {
        let brick = gtk::gdk::RGBA::new(192.0 / 255.0, 57.0 / 255.0, 43.0 / 255.0, 1.0);

        assert_eq!(rgb_of(&brick), Rgb::new(0xc0, 0x39, 0x2b));
    }

    #[test]
    fn a_custom_color_is_shown_as_an_uppercase_code() {
        assert_eq!(code_of(Rgb::new(0xc0, 0x39, 0x2b)), "#C0392B");
    }

    #[test]
    fn the_checkmark_stays_readable_on_light_and_dark_colors() {
        assert_eq!(ink_on(Rgb::new(0xff, 0xff, 0xff)), DARK_INK);
        assert_eq!(ink_on(Rgb::new(0xf6, 0xd3, 0x2d)), DARK_INK);
        assert_eq!(ink_on(Rgb::new(0xc0, 0x39, 0x2b)), LIGHT_INK);
        assert_eq!(ink_on(Rgb::new(0x1a, 0x3a, 0x6b)), LIGHT_INK);
    }
}
