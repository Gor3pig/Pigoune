use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;
use pigoune_core::Rgb;

use crate::custom_color_swatch::{self, CustomColorSwatch};

const CHECKMARK_ICON: &str = "object-select-symbolic";
const SWATCHES_PER_LINE: u32 = 7;

type PickedCallback = Box<dyn Fn(Rgb)>;

thread_local! {
    static NEXT_ROW: Cell<u32> = const { Cell::new(0) };
}

pub struct ColorSwatches {
    flow: gtk::FlowBox,
    buttons: Vec<(Rgb, gtk::ToggleButton)>,
    custom: Rc<CustomColorSwatch>,
    provider: gtk::CssProvider,
    quiet: Cell<bool>,
    on_picked: RefCell<Option<PickedCallback>>,
}

impl ColorSwatches {
    pub fn new(colors: &[(Rgb, String)]) -> Rc<Self> {
        let row = NEXT_ROW.with(|next| {
            let number = next.get();
            next.set(number + 1);
            number
        });
        let flow = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .min_children_per_line(SWATCHES_PER_LINE)
            .max_children_per_line(SWATCHES_PER_LINE)
            .homogeneous(true)
            .row_spacing(6)
            .build();
        let mut styles = String::new();
        let buttons = colors
            .iter()
            .enumerate()
            .map(|(index, (color, name))| {
                let class = format!("wallpaper-swatch-{row}-{index}");
                let hex = color.hex();
                for part in [
                    ".collection-swatch.",
                    class.as_str(),
                    " { background-color: #",
                    hex.as_str(),
                    "; color: ",
                    custom_color_swatch::ink_on(*color),
                    "; box-shadow: inset 0 0 0 1px var(--border-color); }\n",
                ] {
                    styles.push_str(part);
                }
                let button = gtk::ToggleButton::builder()
                    .child(&gtk::Image::from_icon_name(CHECKMARK_ICON))
                    .tooltip_text(name)
                    .halign(gtk::Align::Center)
                    .css_classes(["collection-swatch", "resource-swatch", class.as_str()])
                    .build();
                button.update_property(&[gtk::accessible::Property::Label(name)]);
                flow.append(&button);
                (*color, button)
            })
            .collect();
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&styles);
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }
        let custom = CustomColorSwatch::new();
        flow.append(custom.button());
        let swatches = Rc::new(Self {
            flow,
            buttons,
            custom,
            provider,
            quiet: Cell::new(false),
            on_picked: RefCell::new(None),
        });
        for (color, button) in &swatches.buttons {
            let color = *color;
            let weak = Rc::downgrade(&swatches);
            button.connect_toggled(move |_| {
                if let Some(swatches) = weak.upgrade() {
                    swatches.picked(color);
                }
            });
        }
        let weak = Rc::downgrade(&swatches);
        swatches.custom.connect_changed(move || {
            if let Some(swatches) = weak.upgrade()
                && let Some(color) = swatches.custom.color()
            {
                swatches.picked(color);
            }
        });
        swatches
    }

    pub fn widget(&self) -> &gtk::FlowBox {
        &self.flow
    }

    pub fn connect_picked(&self, callback: impl Fn(Rgb) + 'static) {
        self.on_picked.replace(Some(Box::new(callback)));
    }

    pub fn show_chosen(&self, chosen: &[Rgb]) {
        self.quiet.set(true);
        for (color, button) in &self.buttons {
            button.set_active(chosen.contains(color));
        }
        if self
            .custom
            .color()
            .is_some_and(|color| !chosen.contains(&color))
        {
            self.custom.set_color(None);
        }
        self.quiet.set(false);
    }

    fn picked(&self, color: Rgb) {
        if self.quiet.get() {
            return;
        }
        let callback = self.on_picked.borrow();
        if let Some(callback) = callback.as_ref() {
            callback(color);
        }
    }
}

impl Drop for ColorSwatches {
    fn drop(&mut self) {
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_remove_provider_for_display(&display, &self.provider);
        }
    }
}
