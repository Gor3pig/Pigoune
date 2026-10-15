use std::rc::Rc;

use adw::prelude::*;
use gettextrs::gettext;
use pigoune_core::annotations::{
    Color, DefaultValue, OptionKind, OptionSpec, Options, ToolDefinition, Value, color_to_value,
    number_to_value, value_to_color,
};

use crate::editor_palette;

#[derive(Debug, Clone)]
pub enum OptionEvent {
    Changed(&'static str, Value),
    SliderPressed,
    SliderReleased,
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
        "dash" => gettext("Line Style"),
        "head" => gettext("Arrowhead"),
        "head_size" => gettext("Arrowhead Size"),
        "corner_radius" => gettext("Corner Radius"),
        "fill" => gettext("Fill Shape"),
        "fill_opacity" => gettext("Fill Opacity"),
        "smooth" => gettext("Smooth Line"),
        "size" => gettext("Badge Size"),
        "style" => gettext("Badge Style"),
        "radius" => gettext("Blur Radius"),
        "cell" => gettext("Cell Size"),
        _ => return None,
    })
}

pub fn choice_label(id: &str, choice: &str) -> Option<String> {
    Some(match (id, choice) {
        ("dash", "solid") => gettext("Solid Line"),
        ("dash", "dashed") => gettext("Dashed Line"),
        ("dash", "dotted") => gettext("Dotted Line"),
        ("head", "triangle") => gettext("Filled Triangle"),
        ("head", "open") => gettext("Open Head"),
        ("head", "double") => gettext("Both Ends"),
        ("head", "none") => gettext("No Head"),
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
    let swatches = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    swatches.set_margin_top(12);
    swatches.set_margin_bottom(12);
    swatches.set_margin_start(12);
    swatches.set_margin_end(12);
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
    row.set_child(Some(&swatches));
    let setter = move |value: &Value| {
        if let Some(color) = value_to_color(value)
            && let Some(index) = editor_palette::index_of(color)
            && let Some(button) = buttons.get(index)
        {
            button.set_active(true);
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
    let click = gtk::GestureClick::new();
    click.set_propagation_phase(gtk::PropagationPhase::Capture);
    let pressed = notify.clone();
    click.connect_pressed(move |_, _, _, _| pressed(OptionEvent::SliderPressed));
    let released = notify.clone();
    click.connect_released(move |_, _, _, _| released(OptionEvent::SliderReleased));
    scale.add_controller(click);
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
    use pigoune_core::annotations::{OptionKind, definitions};

    use super::{choice_label, option_label};

    #[test]
    fn every_option_of_every_tool_has_a_label() {
        for definition in definitions() {
            for spec in definition.options {
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
