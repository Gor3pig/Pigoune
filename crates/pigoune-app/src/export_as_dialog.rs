use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};

use crate::image_conversion::{ConversionSettings, TargetFormat};

const COLOR_CHANNEL_MAX: f32 = 255.0;

type ExportCallback = Box<dyn Fn(ConversionSettings)>;

mod imp {
    use std::cell::RefCell;

    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::ExportCallback;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/gor3pig/Pigoune/ui/export-as-dialog.ui")]
    pub struct PigouneExportAsDialog {
        #[template_child]
        pub format_row: TemplateChild<adw::ComboRow>,
        #[template_child]
        pub quality_row: TemplateChild<adw::SpinRow>,
        #[template_child]
        pub background_row: TemplateChild<adw::ActionRow>,
        #[template_child]
        pub background_button: TemplateChild<gtk::ColorDialogButton>,
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
        on_export: impl Fn(ConversionSettings) + 'static,
    ) -> Self {
        let dialog: Self = glib::Object::new();
        let imp = dialog.imp();
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
        }
    }

    #[template_callback]
    fn on_format_changed(&self) {
        let imp = self.imp();
        let format = self.format();
        imp.format_row.set_subtitle(&description(format));
        imp.quality_row.set_visible(format.has_quality());
        imp.background_row.set_visible(!format.keeps_transparency());
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

fn description(format: TargetFormat) -> String {
    match format {
        TargetFormat::Png | TargetFormat::Webp => gettext("Lossless, keeps transparency"),
        TargetFormat::Jpeg => gettext("Small files, no transparency"),
        TargetFormat::Avif => gettext("Small files, keeps transparency"),
        TargetFormat::Ico => gettext("Icons up to 256 × 256 pixels, keeps transparency"),
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
