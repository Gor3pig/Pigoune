use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::Dimensions;

use crate::export_size::{CustomSize, Framing, ScreenSize, SizeUnit};
use crate::icon_sides::ICON_SIDES;
use crate::image_conversion::{ConversionSettings, TargetFormat};

const COLOR_CHANNEL_MAX: f32 = 255.0;
const LARGEST_PIXELS: f64 = 32768.0;
const LOCKED_ICON: &str = "changes-prevent-symbolic";
const UNLOCKED_ICON: &str = "changes-allow-symbolic";
const LARGEST_PERCENT: f64 = 1000.0;
const FIT: &str = "fit";

type ExportCallback = Box<dyn Fn(ConversionSettings)>;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use pigoune_core::Dimensions;

    use super::ExportCallback;
    use crate::export_size::CustomSize;
    use crate::icon_sides::IconSides;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/export-as-dialog.ui")]
    pub struct PigouneExportAsDialog {
        #[template_child]
        pub format_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub export_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub screen_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub framing_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub framing_toggles: TemplateChild<adw::ToggleGroup>,
        #[template_child]
        pub dimensions_row: TemplateChild<gtk::ListBoxRow>,
        #[template_child]
        pub width_spin: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub height_spin: TemplateChild<gtk::SpinButton>,
        #[template_child]
        pub link_button: TemplateChild<gtk::ToggleButton>,
        #[template_child]
        pub several_hint: TemplateChild<gtk::Label>,
        #[template_child]
        pub unit_dropdown: TemplateChild<gtk::DropDown>,
        #[template_child]
        pub icon_sides_row: TemplateChild<gtk::ListBoxRow>,
        #[template_child]
        pub icon_sides_box: TemplateChild<gtk::FlowBox>,
        #[template_child]
        pub quality_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub transparency_row: TemplateChild<adw::SwitchRow>,
        #[template_child]
        pub background_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub background_button: TemplateChild<gtk::ColorDialogButton>,
        pub icon_sides: Cell<IconSides>,
        pub custom: Cell<CustomSize>,
        pub reference: Cell<(u32, u32)>,
        pub screen: Cell<Option<Dimensions>>,
        pub showing_custom: Cell<bool>,
        pub on_export: RefCell<Option<ExportCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneExportAsDialog {
        const NAME: &'static str = "PigouneExportAsDialog";
        type Type = super::PigouneExportAsDialog;
        type ParentType = adw::Dialog;

        fn class_init(class: &mut Self::Class) {
            class.bind_template();
            class.bind_template_instance_callbacks();
        }

        fn instance_init(object: &glib::subclass::InitializingObject<Self>) {
            object.init_template();
        }
    }

    impl ObjectImpl for PigouneExportAsDialog {}
    impl WidgetImpl for PigouneExportAsDialog {}
    impl AdwDialogImpl for PigouneExportAsDialog {}
}

glib::wrapper! {
    pub struct PigouneExportAsDialog(ObjectSubclass<imp::PigouneExportAsDialog>)
        @extends adw::Dialog, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::ShortcutManager;
}

#[gtk::template_callbacks]
impl PigouneExportAsDialog {
    pub fn new(
        settings: ConversionSettings,
        reference: (u32, u32),
        resource_count: usize,
        screen: Option<Dimensions>,
        on_export: impl Fn(ConversionSettings) + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
        imp.screen.set(screen);
        if let Some(screen) = screen {
            imp.screen_row
                .set_subtitle(&format!("{} × {}", screen.width(), screen.height()));
        }
        imp.several_hint.set_visible(resource_count > 1);
        let names: Vec<&str> = TargetFormat::ALL
            .iter()
            .map(|format| format.name())
            .collect();
        imp.format_row
            .set_model(Some(&gtk::StringList::new(&names)));
        let position = TargetFormat::ALL
            .iter()
            .position(|format| *format == settings.format)
            .unwrap_or(0);
        imp.format_row
            .set_selected(u32::try_from(position).unwrap_or(0));
        imp.quality_row.set_value(f64::from(settings.quality));
        imp.reference.set(reference);
        dialog.show_custom(settings.custom);
        imp.icon_sides.set(settings.icon_sides);
        imp.transparency_row.set_active(settings.keep_transparency);
        dialog.offer_icon_sides();
        imp.background_button
            .set_rgba(&rgba_of(settings.background));
        imp.on_export.replace(Some(Box::new(on_export)));
        dialog.on_format_changed();
        dialog
    }

    fn format(&self) -> TargetFormat {
        let selected = usize::try_from(self.imp().format_row.selected()).unwrap_or(0);
        TargetFormat::ALL
            .get(selected)
            .copied()
            .unwrap_or(TargetFormat::Png)
    }

    fn settings(&self) -> ConversionSettings {
        let imp = self.imp();
        ConversionSettings {
            format: self.format(),
            quality: byte_of(imp.quality_row.value()),
            background: channels_of(&imp.background_button.rgba()),
            custom: imp.custom.get(),
            screen: self.chosen_screen(),
            keep_transparency: imp.transparency_row.is_active(),
            icon_sides: imp.icon_sides.get(),
        }
    }

    fn chosen_screen(&self) -> Option<ScreenSize> {
        let imp = self.imp();
        let screen = imp.screen.get()?;
        let shown = self.format() != TargetFormat::Ico && imp.screen_row.is_active();
        shown.then(|| ScreenSize {
            width: screen.width(),
            height: screen.height(),
            framing: self.framing(),
        })
    }

    fn framing(&self) -> Framing {
        if self.imp().framing_toggles.active_name().as_deref() == Some(FIT) {
            Framing::Fit
        } else {
            Framing::Fill
        }
    }

    fn offer_icon_sides(&self) {
        let imp = self.imp();
        for side in ICON_SIDES {
            let check = gtk::CheckButton::with_label(&side.to_string());
            check.set_active(imp.icon_sides.get().contains(side));
            check.connect_toggled(glib::clone!(
                #[weak(rename_to = dialog)]
                self,
                move |check| {
                    let imp = dialog.imp();
                    imp.icon_sides
                        .set(imp.icon_sides.get().with(side, check.is_active()));
                    dialog.refresh_rows();
                }
            ));
            imp.icon_sides_box.append(&check);
        }
    }

    #[template_callback]
    fn on_format_changed(&self) {
        let imp = self.imp();
        imp.format_row.set_subtitle(&description(self.format()));
        self.refresh_rows();
    }

    fn refresh_rows(&self) {
        let imp = self.imp();
        let format = self.format();
        let is_icon = format == TargetFormat::Ico;
        let on_screen = self.chosen_screen().is_some();
        imp.screen_row
            .set_visible(!is_icon && imp.screen.get().is_some());
        imp.framing_row.set_visible(on_screen);
        imp.framing_row.set_subtitle(&framing_text(self.framing()));
        imp.dimensions_row.set_visible(!is_icon && !on_screen);
        imp.icon_sides_row.set_visible(is_icon);
        imp.quality_row.set_visible(format.has_quality());
        imp.transparency_row
            .set_visible(format.keeps_transparency());
        imp.background_row
            .set_visible(!format.keeps_transparency() || !imp.transparency_row.is_active());
        imp.export_button
            .set_sensitive(!is_icon || !imp.icon_sides.get().is_empty());
    }

    fn show_custom(&self, custom: CustomSize) {
        let imp = self.imp();
        imp.custom.set(custom);
        imp.several_hint.set_label(&several_text(custom));
        imp.showing_custom.set(true);
        let largest = match custom.unit {
            SizeUnit::Pixels => LARGEST_PIXELS,
            SizeUnit::Percent => LARGEST_PERCENT,
        };
        for (spin, value) in [
            (&imp.width_spin, custom.width),
            (&imp.height_spin, custom.height),
        ] {
            spin.adjustment().set_upper(largest);
            spin.set_value(value);
        }
        imp.link_button.set_active(custom.linked);
        imp.link_button.set_icon_name(if custom.linked {
            LOCKED_ICON
        } else {
            UNLOCKED_ICON
        });
        imp.unit_dropdown
            .set_selected(u32::from(custom.unit == SizeUnit::Percent));
        imp.showing_custom.set(false);
    }

    fn change_custom(&self, change: impl FnOnce(CustomSize, (u32, u32)) -> CustomSize) {
        let imp = self.imp();
        if imp.showing_custom.get() {
            return;
        }
        self.show_custom(change(imp.custom.get(), imp.reference.get()));
    }

    #[template_callback]
    fn on_width_changed(&self) {
        let width = self.imp().width_spin.value();
        self.change_custom(|custom, reference| custom.with_width(width, reference));
    }

    #[template_callback]
    fn on_height_changed(&self) {
        let height = self.imp().height_spin.value();
        self.change_custom(|custom, reference| custom.with_height(height, reference));
    }

    #[template_callback]
    fn on_link_toggled(&self) {
        let linked = self.imp().link_button.is_active();
        self.change_custom(|custom, reference| custom.with_linked(linked, reference));
    }

    #[template_callback]
    fn on_unit_changed(&self) {
        let unit = if self.imp().unit_dropdown.selected() == 0 {
            SizeUnit::Pixels
        } else {
            SizeUnit::Percent
        };
        self.change_custom(|custom, reference| custom.in_unit(unit, reference));
    }

    #[template_callback]
    fn on_screen_changed(&self) {
        self.refresh_rows();
    }

    #[template_callback]
    fn on_framing_changed(&self) {
        self.refresh_rows();
    }

    #[template_callback]
    fn on_transparency_changed(&self) {
        self.refresh_rows();
    }

    #[template_callback]
    fn on_cancel_clicked(&self) {
        self.close();
    }

    #[template_callback]
    fn on_export_clicked(&self) {
        let settings = self.settings();
        self.close();
        if let Some(on_export) = self.imp().on_export.borrow().as_ref() {
            on_export(settings);
        }
    }
}

fn several_text(custom: CustomSize) -> String {
    match (custom.unit, custom.linked) {
        (SizeUnit::Percent, _) => gettext("Each resource is resized by this percentage"),
        (SizeUnit::Pixels, true) => {
            gettext("Each resource fits inside this frame, without being stretched")
        }
        (SizeUnit::Pixels, false) => gettext("Each resource takes exactly this size"),
    }
}

fn framing_text(framing: Framing) -> String {
    match framing {
        Framing::Fill => gettext("Covers the whole screen; the overflowing edges are cut"),
        Framing::Fit => gettext("Keeps the whole image, with bands filling the rest"),
    }
}

fn description(format: TargetFormat) -> String {
    match format {
        TargetFormat::Png | TargetFormat::Webp => gettext("Lossless, supports transparency"),
        TargetFormat::Jpeg => gettext("Small files, no transparency"),
        TargetFormat::Avif => gettext("Small files, supports transparency"),
        TargetFormat::Ico => gettext("One icon file holding several sizes"),
    }
}

fn rgba_of(channels: [u8; 3]) -> gdk::RGBA {
    let [red, green, blue] = channels.map(|channel| f32::from(channel) / COLOR_CHANNEL_MAX);
    gdk::RGBA::new(red, green, blue, 1.0)
}

fn channels_of(color: &gdk::RGBA) -> [u8; 3] {
    [color.red(), color.green(), color.blue()]
        .map(|channel| byte_of(f64::from(channel.clamp(0.0, 1.0) * COLOR_CHANNEL_MAX)))
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the value is rounded and clamped to the range of a byte"
)]
fn byte_of(value: f64) -> u8 {
    value.round().clamp(0.0, f64::from(u8::MAX)) as u8
}
