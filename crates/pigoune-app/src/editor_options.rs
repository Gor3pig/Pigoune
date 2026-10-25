use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use gtk::{gdk, glib};
use pigoune_core::annotations::{
    Color, DefaultValue, OptionKind, OptionSpec, Options, ToolDefinition, Value, color_to_value,
    is_set_by_handle, number_to_value, value_to_color,
};

use crate::editor_palette;

#[derive(Debug, Clone)]
pub enum OptionEvent {
    Changed(&'static str, Value),
    SeriesBegin,
    SeriesEnd,
    PickColor(&'static str),
}

pub fn color_value(color: Color) -> Value {
    color_to_value(color)
}

pub type Notify = Rc<dyn Fn(OptionEvent)>;
type Setter = Box<dyn Fn(&Value)>;
type BuiltRow = (gtk::Widget, Setter);

pub struct OptionRow {
    pub id: &'static str,
    pub widget: gtk::Widget,
    shown_if: Option<&'static str>,
    spec: &'static OptionSpec,
    setter: Setter,
}

impl OptionRow {
    pub fn show(&self, value: &Value) {
        (self.setter)(value);
    }
}

pub fn option_label(id: &str) -> Option<String> {
    Some(match id {
        "color" => gettext("Color"),
        "opacity" => gettext("Opacity"),
        "width" => gettext("Thickness"),
        "dash" => gettext("Line Type"),
        "roughness" => gettext("Hand-Drawn Look"),
        "curve" => gettext("Curve"),
        "sequence" => gettext("Badge Content"),
        "outline" => gettext("Text Outline"),
        "shadow" => gettext("Shadow"),
        "shaft" => gettext("Shaft"),
        "spot" => gettext("Spotlight Shape"),
        "dimming" => gettext("Dimming"),
        "head" => gettext("Arrowhead"),
        "head_size" => gettext("Arrowhead Size"),
        "corner_radius" => gettext("Corner Radius"),
        "fill" => gettext("Fill Shape"),
        "fill_opacity" => gettext("Fill Opacity"),
        "smooth" => gettext("Smooth Line"),
        "size" => gettext("Badge Size"),
        "rotation" => gettext("Rotation"),
        "style" => gettext("Badge Style"),
        "font_size" => gettext("Font Size"),
        "bold" => gettext("Bold Text"),
        "italic" => gettext("Italic Text"),
        "family" => gettext("Font"),
        "background" => gettext("Text Background"),
        "radius" => gettext("Blur Radius"),
        "cell" => gettext("Cell Size"),
        "form" => gettext("Bubble Shape"),
        "lettering" => gettext("Bubble Lettering"),
        "bumps" => gettext("Cloud Bumps"),
        "shadow_style" => gettext("Shadow Style"),
        "fill_color" => gettext("Bubble Fill"),
        _ => return None,
    })
}

pub fn choice_label(id: &str, choice: &str) -> Option<String> {
    Some(match (id, choice) {
        ("shadow_style", "soft") => gettext("Soft Shadow"),
        ("shadow_style", "hard") => gettext("Hard Shadow"),
        ("bumps", "small") => gettext("Small Bumps"),
        ("bumps", "medium") => gettext("Medium Bumps"),
        ("lettering", "cartoon") => gettext("Cartoon Lettering"),
        ("lettering", "normal") => gettext("Normal Lettering"),
        ("form", "oval") => gettext("Oval Bubble"),
        ("form", "rounded") => gettext("Rounded Bubble"),
        ("form", "cloud") => gettext("Cloud Bubble"),
        ("sequence", "numbers") => gettext("Numbers"),
        ("sequence", "letters") => gettext("Letters"),
        ("background", "bubble") => gettext("Bubble Background"),
        ("shaft", "even") => gettext("Even Shaft"),
        ("shaft", "tapered") => gettext("Tapered Shaft"),
        ("spot", "rectangle") => gettext("Rectangular"),
        ("spot", "ellipse") => gettext("Oval"),
        ("spot", "rounded") => gettext("Rounded"),
        ("roughness", "precise") => gettext("Precise"),
        ("roughness", "sketchy") => gettext("Sketchy"),
        ("roughness", "very_sketchy") => gettext("Very Sketchy"),
        ("dash", "solid") => gettext("Solid Line"),
        ("dash", "dashed") => gettext("Dashed Line"),
        ("dash", "dotted") => gettext("Dotted Line"),
        ("head", "triangle") => gettext("Filled Triangle"),
        ("head", "open") => gettext("Open Head"),
        ("head", "double") => gettext("Both Ends"),
        ("head", "none") => gettext("No Head"),
        ("family", "sans") => gettext("Sans-serif Font"),
        ("family", "serif") => gettext("Serif Font"),
        ("family", "mono") => gettext("Monospace Font"),
        ("background", "none") => gettext("No Background"),
        ("background", "light") => gettext("Light Background"),
        ("background", "dark") => gettext("Dark Background"),
        ("style", "filled") => gettext("Filled Badge"),
        ("style", "outlined") => gettext("Outlined Badge"),
        ("head_size", "small") => gettext("Small Head"),
        ("head_size", "medium") => gettext("Medium Head"),
        ("head_size", "large") => gettext("Large Head"),
        _ => return None,
    })
}

pub fn build_rows(definition: &ToolDefinition, notify: &Notify) -> Vec<OptionRow> {
    definition
        .options
        .iter()
        .filter(|spec| spec.kind != OptionKind::Text && !is_set_by_handle(spec.id))
        .map(|spec| build_row(spec, notify))
        .collect()
}

pub fn show_values(rows: &[OptionRow], options: &Options) {
    for row in rows {
        let value = options
            .get(row.id)
            .filter(|value| row.spec.accepts(value))
            .cloned()
            .unwrap_or_else(|| row.spec.default_value());
        row.show(&value);
    }
    update_visibility(rows, options);
}

pub fn update_visibility(rows: &[OptionRow], options: &Options) {
    for row in rows {
        if let Some(controller) = row.shown_if {
            let shown = rows
                .iter()
                .find(|other| other.id == controller)
                .is_some_and(|other| options.toggle(other.spec));
            row.widget.set_visible(shown);
        }
    }
}

fn build_row(spec: &'static OptionSpec, notify: &Notify) -> OptionRow {
    let (widget, setter): BuiltRow = match spec.kind {
        OptionKind::Color => color_row(spec, notify),
        OptionKind::Opacity => slider_row(spec, 5.0, 100.0, 100.0, notify),
        OptionKind::Number { min, max, .. } => slider_row(spec, min, max, 1.0, notify),
        OptionKind::Choice(choices) => choice_row(spec, choices, notify),
        OptionKind::Toggle => toggle_row(spec, notify),
        OptionKind::Text => unreachable!("a text option has no row"),
    };
    OptionRow {
        id: spec.id,
        widget,
        shown_if: spec.shown_if,
        spec,
        setter,
    }
}

fn title_of(spec: &OptionSpec) -> String {
    option_label(spec.id).unwrap_or_else(|| spec.id.to_owned())
}

fn color_row(spec: &'static OptionSpec, notify: &Notify) -> BuiltRow {
    let row = gtk::ListBoxRow::new();
    row.set_activatable(false);
    row.set_selectable(false);
    let swatches = gtk::FlowBox::new();
    swatches.set_selection_mode(gtk::SelectionMode::None);
    swatches.set_homogeneous(false);
    swatches.add_css_class("annotation-swatches");
    swatches.set_column_spacing(4);
    swatches.set_row_spacing(6);
    swatches.set_max_children_per_line(16);
    swatches.set_margin_top(12);
    swatches.set_margin_bottom(12);
    swatches.set_margin_start(10);
    swatches.set_margin_end(10);
    swatches.set_halign(gtk::Align::Start);
    let mut first: Option<gtk::ToggleButton> = None;
    let mut buttons = Vec::new();
    for (index, color) in editor_palette::COLORS.iter().enumerate() {
        let button = gtk::ToggleButton::new();
        button.add_css_class("annotation-swatch");
        button.add_css_class(&format!("swatch-{index}"));
        let name = editor_palette::color_name(index);
        button.set_tooltip_text(Some(&name));
        button.update_property(&[gtk::accessible::Property::Label(&name)]);
        match &first {
            Some(first) => button.set_group(Some(first)),
            None => first = Some(button.clone()),
        }
        let color: Color = *color;
        let notify = notify.clone();
        button.connect_toggled(move |button| {
            if button.is_active() {
                notify(OptionEvent::Changed(spec.id, color_to_value(color)));
            }
        });
        swatches.append(&button);
        buttons.push(button);
    }
    let unlisted = gtk::ToggleButton::new();
    if let Some(first) = &first {
        unlisted.set_group(Some(first));
    }
    let picker = gtk::Button::from_icon_name("eyedropper-symbolic");
    picker.add_css_class("flat");
    picker.add_css_class("circular");
    picker.add_css_class("annotation-picker");
    picker.set_valign(gtk::Align::Center);
    let tooltip = gettext("Pick a Color from the Image");
    picker.set_tooltip_text(Some(&tooltip));
    picker.update_property(&[gtk::accessible::Property::Label(&tooltip)]);
    let asking = notify.clone();
    picker.connect_clicked(move |_| asking(OptionEvent::PickColor(spec.id)));
    swatches.append(&picker);
    row.set_child(Some(&swatches));
    let setter = move |value: &Value| {
        if let Some(color) = value_to_color(value)
            && let Some(index) = editor_palette::index_of(color)
            && let Some(button) = buttons.get(index)
        {
            button.set_active(true);
        } else {
            unlisted.set_active(true);
        }
    };
    (row.upcast(), Box::new(setter))
}

fn slider_row(
    spec: &'static OptionSpec,
    min: f32,
    max: f32,
    factor: f32,
    notify: &Notify,
) -> BuiltRow {
    let row = adw::ActionRow::new();
    row.set_title(&title_of(spec));
    let scale = gtk::Scale::with_range(
        gtk::Orientation::Horizontal,
        f64::from(min),
        f64::from(max),
        1.0,
    );
    scale.set_width_request(140);
    scale.set_valign(gtk::Align::Center);
    scale.set_draw_value(false);
    let default = match spec.default {
        DefaultValue::Number(number) => number * factor,
        _ => min,
    };
    scale.set_value(f64::from(default));
    let sender = notify.clone();
    scale.connect_value_changed(move |scale| {
        let value = narrow(scale.value()) / factor;
        sender(OptionEvent::Changed(spec.id, number_to_value(value)));
    });
    let bracket = gtk::EventControllerLegacy::new();
    bracket.set_propagation_phase(gtk::PropagationPhase::Capture);
    let bracketed = notify.clone();
    bracket.connect_event(move |_, event| {
        match event.event_type() {
            gdk::EventType::ButtonPress | gdk::EventType::KeyPress => {
                bracketed(OptionEvent::SeriesBegin);
            }
            gdk::EventType::ButtonRelease | gdk::EventType::KeyRelease => {
                bracketed(OptionEvent::SeriesEnd);
            }
            _ => {}
        }
        glib::Propagation::Proceed
    });
    scale.add_controller(bracket);
    let unmapped = notify.clone();
    scale.connect_unmap(move |_| unmapped(OptionEvent::SeriesEnd));
    row.add_suffix(&scale);
    let setter = move |value: &Value| {
        if let Some(number) = value.as_f64() {
            scale.set_value(number * f64::from(factor));
        }
    };
    (row.upcast(), Box::new(setter))
}

fn choice_row(
    spec: &'static OptionSpec,
    choices: &'static [&'static str],
    notify: &Notify,
) -> BuiltRow {
    let labels: Vec<String> = choices
        .iter()
        .map(|choice| choice_label(spec.id, choice).unwrap_or_else(|| (*choice).to_owned()))
        .collect();
    let names: Vec<&str> = labels.iter().map(String::as_str).collect();
    let row = adw::ComboRow::new();
    row.set_title(&title_of(spec));
    row.set_model(Some(&gtk::StringList::new(&names)));
    let sender = notify.clone();
    row.connect_selected_notify(move |row| {
        if let Some(choice) = choices.get(row.selected() as usize) {
            sender(OptionEvent::Changed(spec.id, Value::from(*choice)));
        }
    });
    let shown = row.clone();
    let setter = move |value: &Value| {
        if let Some(index) = value
            .as_str()
            .and_then(|choice| choices.iter().position(|candidate| *candidate == choice))
            && let Ok(index) = u32::try_from(index)
        {
            shown.set_selected(index);
        }
    };
    (row.upcast(), Box::new(setter))
}

fn toggle_row(spec: &'static OptionSpec, notify: &Notify) -> BuiltRow {
    let row = adw::SwitchRow::new();
    row.set_title(&title_of(spec));
    let sender = notify.clone();
    row.connect_active_notify(move |row| {
        sender(OptionEvent::Changed(spec.id, Value::from(row.is_active())));
    });
    let shown = row.clone();
    let setter = move |value: &Value| {
        if let Some(active) = value.as_bool() {
            shown.set_active(active);
        }
    };
    (row.upcast(), Box::new(setter))
}

fn narrow(value: f64) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "slider values are small numbers that fit in f32"
    )]
    let narrowed = value as f32;
    narrowed
}

#[cfg(test)]
mod tests {
    use pigoune_core::annotations::{OptionKind, definitions, is_set_by_handle};

    use super::{choice_label, option_label};

    #[test]
    fn every_option_of_every_tool_has_a_label() {
        for definition in definitions() {
            for spec in definition.options {
                if spec.kind == OptionKind::Text || is_set_by_handle(spec.id) {
                    continue;
                }
                assert!(
                    option_label(spec.id).is_some(),
                    "{} / {} has no label",
                    definition.kind,
                    spec.id
                );
            }
        }
    }

    #[test]
    fn every_choice_of_every_option_has_a_label() {
        for definition in definitions() {
            for spec in definition.options {
                if let OptionKind::Choice(choices) = spec.kind {
                    for choice in choices {
                        assert!(
                            choice_label(spec.id, choice).is_some(),
                            "{} / {} / {choice} has no label",
                            definition.kind,
                            spec.id
                        );
                    }
                }
            }
        }
    }
}
