use serde_json::{Map, Value};

use super::Color;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DefaultValue {
    Color(Color),
    Number(f32),
    Choice(&'static str),
    Toggle(bool),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OptionKind {
    Color,
    Opacity,
    Number { min: f32, max: f32, step: f32 },
    Choice(&'static [&'static str]),
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OptionSpec {
    pub id: &'static str,
    pub kind: OptionKind,
    pub default: DefaultValue,
    pub shown_if: Option<&'static str>,
}

impl OptionSpec {
    #[must_use]
    pub fn default_value(&self) -> Value {
        match self.default {
            DefaultValue::Color(color) => color_to_value(color),
            DefaultValue::Number(number) => number_to_value(number),
            DefaultValue::Choice(choice) => Value::from(choice),
            DefaultValue::Toggle(toggle) => Value::from(toggle),
        }
    }

    #[must_use]
    pub fn accepts(&self, value: &Value) -> bool {
        match self.kind {
            OptionKind::Color => value_to_color(value).is_some(),
            OptionKind::Opacity => value.as_f64().is_some_and(|n| (0.0..=1.0).contains(&n)),
            OptionKind::Number { min, max, .. } => value
                .as_f64()
                .is_some_and(|n| n >= f64::from(min) && n <= f64::from(max)),
            OptionKind::Choice(choices) => value
                .as_str()
                .is_some_and(|choice| choices.contains(&choice)),
            OptionKind::Toggle => value.is_boolean(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Options(Map<String, Value>);

impl Options {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn defaults(specs: &[OptionSpec]) -> Self {
        Self(
            specs
                .iter()
                .map(|spec| (spec.id.to_owned(), spec.default_value()))
                .collect(),
        )
    }

    #[must_use]
    pub fn from_map(map: Map<String, Value>) -> Self {
        Self(map)
    }

    #[must_use]
    pub fn as_map(&self) -> &Map<String, Value> {
        &self.0
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Value> {
        self.0.get(id)
    }

    pub fn set(&mut self, id: &str, value: Value) {
        self.0.insert(id.to_owned(), value);
    }

    #[must_use]
    pub fn with(mut self, id: &str, value: Value) -> Self {
        self.set(id, value);
        self
    }

    #[must_use]
    pub fn color(&self, spec: &OptionSpec) -> Color {
        let fallback = match spec.default {
            DefaultValue::Color(color) => color,
            _ => BLACK,
        };
        self.get(spec.id)
            .and_then(value_to_color)
            .unwrap_or(fallback)
    }

    #[must_use]
    pub fn number(&self, spec: &OptionSpec) -> f32 {
        let fallback = match spec.default {
            DefaultValue::Number(number) => number,
            _ => 0.0,
        };
        let value = self
            .get(spec.id)
            .and_then(Value::as_f64)
            .map_or(fallback, narrow);
        match spec.kind {
            OptionKind::Number { min, max, .. } => value.clamp(min, max),
            OptionKind::Opacity => value.clamp(0.0, 1.0),
            _ => value,
        }
    }

    #[must_use]
    pub fn choice<'a>(&'a self, spec: &'a OptionSpec) -> &'a str {
        let fallback = match spec.default {
            DefaultValue::Choice(choice) => choice,
            _ => "",
        };
        let choices: &[&str] = match spec.kind {
            OptionKind::Choice(choices) => choices,
            _ => &[],
        };
        self.get(spec.id)
            .and_then(Value::as_str)
            .filter(|choice| choices.contains(choice))
            .unwrap_or(fallback)
    }

    #[must_use]
    pub fn toggle(&self, spec: &OptionSpec) -> bool {
        let fallback = matches!(spec.default, DefaultValue::Toggle(true));
        self.get(spec.id)
            .and_then(Value::as_bool)
            .unwrap_or(fallback)
    }
}

const BLACK: Color = Color {
    r: 0,
    g: 0,
    b: 0,
    a: 255,
};

#[expect(
    clippy::cast_possible_truncation,
    reason = "option values are small numbers that fit in f32"
)]
fn narrow(value: f64) -> f32 {
    value as f32
}

#[must_use]
pub fn color_to_value(color: Color) -> Value {
    Value::from(vec![color.r, color.g, color.b, color.a])
}

#[must_use]
pub fn value_to_color(value: &Value) -> Option<Color> {
    let parts = value.as_array()?;
    if parts.len() != 4 {
        return None;
    }
    let channel = |index: usize| {
        parts[index]
            .as_u64()
            .and_then(|channel| u8::try_from(channel).ok())
    };
    Some(Color {
        r: channel(0)?,
        g: channel(1)?,
        b: channel(2)?,
        a: channel(3)?,
    })
}

#[must_use]
pub fn number_to_value(number: f32) -> Value {
    serde_json::Number::from_f64(f64::from(number)).map_or(Value::Null, Value::Number)
}
