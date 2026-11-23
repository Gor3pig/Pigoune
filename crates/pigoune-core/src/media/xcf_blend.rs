use std::sync::OnceLock;

const LUMA: [f32; 3] = [0.222_504_5, 0.716_878_6, 0.060_616_9];
const D50_WHITE: [f32; 3] = [0.9642, 1.0, 0.8249];
const LINEAR_TO_XYZ: [[f32; 3]; 3] = [
    [0.436_074_7, 0.385_064_9, 0.143_080_4],
    [0.222_504_5, 0.716_878_6, 0.060_616_9],
    [0.013_932_2, 0.097_104_5, 0.714_173_3],
];
const XYZ_TO_LINEAR: [[f32; 3]; 3] = [
    [3.133_856, -1.616_867, -0.490_615],
    [-0.978_768_4, 1.916_141_5, 0.033_454],
    [0.071_945_3, -0.228_991_4, 1.405_242_7],
];
const LAB_EPSILON: f32 = 216.0 / 24_389.0;
const LAB_KAPPA: f32 = 24_389.0 / 27.0;
const NEARLY_ZERO: f32 = 1e-10;

pub(super) type Rgb = [f32; 3];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Space {
    Linear,
    Perceptual,
}

impl Space {
    pub(super) fn from_stored(value: i32) -> Option<Self> {
        match value.unsigned_abs() {
            0 => None,
            1 | 3 => Some(Self::Linear),
            _ => Some(Self::Perceptual),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Composite {
    Union,
    ClipToBackdrop,
    ClipToLayer,
    Intersection,
}

impl Composite {
    pub(super) fn from_stored(value: i32) -> Option<Self> {
        match value.unsigned_abs() {
            1 => Some(Self::Union),
            2 => Some(Self::ClipToBackdrop),
            3 => Some(Self::ClipToLayer),
            4 => Some(Self::Intersection),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Blend {
    Normal,
    Behind,
    Multiply,
    Screen,
    Overlay,
    Difference,
    Addition,
    Subtract,
    DarkenOnly,
    LightenOnly,
    Divide,
    Dodge,
    Burn,
    HardLight,
    SoftLight,
    GrainExtract,
    GrainMerge,
    VividLight,
    PinLight,
    LinearLight,
    HardMix,
    Exclusion,
    LinearBurn,
    HsvHue,
    HsvSaturation,
    HsvColor,
    HsvValue,
    LchHue,
    LchChroma,
    LchColor,
    LchLightness,
    LumaDarkenOnly,
    LumaLightenOnly,
    Luminance,
    Erase,
    PassThrough,
}

impl Blend {
    pub(super) fn from_code(code: u32) -> Option<Self> {
        Some(match code {
            0 | 28 => Self::Normal,
            2 | 29 => Self::Behind,
            3 | 30 => Self::Multiply,
            4 | 31 => Self::Screen,
            5 | 19 | 45 => Self::SoftLight,
            6 | 32 => Self::Difference,
            7 | 33 => Self::Addition,
            8 | 34 => Self::Subtract,
            9 | 35 => Self::DarkenOnly,
            10 | 36 => Self::LightenOnly,
            11 | 37 => Self::HsvHue,
            12 | 38 => Self::HsvSaturation,
            13 | 39 => Self::HsvColor,
            14 | 40 => Self::HsvValue,
            15 | 41 => Self::Divide,
            16 | 42 => Self::Dodge,
            17 | 43 => Self::Burn,
            18 | 44 => Self::HardLight,
            20 | 46 => Self::GrainExtract,
            21 | 47 => Self::GrainMerge,
            23 => Self::Overlay,
            24 => Self::LchHue,
            25 => Self::LchChroma,
            26 => Self::LchColor,
            27 => Self::LchLightness,
            48 => Self::VividLight,
            49 => Self::PinLight,
            50 => Self::LinearLight,
            51 => Self::HardMix,
            52 => Self::Exclusion,
            53 => Self::LinearBurn,
            54 => Self::LumaDarkenOnly,
            55 => Self::LumaLightenOnly,
            56 => Self::Luminance,
            58 => Self::Erase,
            61 => Self::PassThrough,
            _ => return None,
        })
    }

    pub(super) fn default_blend_space(self) -> Space {
        match self {
            Self::Normal
            | Self::Behind
            | Self::Multiply
            | Self::Addition
            | Self::Subtract
            | Self::Divide
            | Self::DarkenOnly
            | Self::LightenOnly
            | Self::LumaDarkenOnly
            | Self::LumaLightenOnly
            | Self::Luminance
            | Self::PassThrough
            | Self::LchHue
            | Self::LchChroma
            | Self::LchColor
            | Self::LchLightness => Space::Linear,
            _ => Space::Perceptual,
        }
    }

    pub(super) fn default_composite(self) -> Composite {
        match self {
            Self::Normal | Self::Behind | Self::PassThrough => Composite::Union,
            _ => Composite::ClipToBackdrop,
        }
    }

    pub(super) fn apply(self, backdrop: Rgb, source: Rgb) -> Rgb {
        match self {
            Self::Normal | Self::Behind | Self::Erase | Self::PassThrough => source,
            Self::HsvHue | Self::HsvSaturation | Self::HsvColor | Self::HsvValue => {
                hsv_blend(self, backdrop, source)
            }
            Self::LchHue | Self::LchChroma | Self::LchColor | Self::LchLightness => {
                lch_blend(self, backdrop, source)
            }
            Self::LumaDarkenOnly => {
                if luma(source) < luma(backdrop) {
                    source
                } else {
                    backdrop
                }
            }
            Self::LumaLightenOnly => {
                if luma(source) > luma(backdrop) {
                    source
                } else {
                    backdrop
                }
            }
            Self::Luminance => luminance_blend(backdrop, source),
            _ => [
                self.channel(backdrop[0], source[0]),
                self.channel(backdrop[1], source[1]),
                self.channel(backdrop[2], source[2]),
            ],
        }
    }

    fn channel(self, b: f32, s: f32) -> f32 {
        match self {
            Self::Multiply => b * s,
            Self::Screen => 1.0 - (1.0 - b) * (1.0 - s),
            Self::Overlay => hard_light(s, b),
            Self::HardLight => hard_light(b, s),
            Self::Difference => (b - s).abs(),
            Self::Addition => b + s,
            Self::Subtract => b - s,
            Self::DarkenOnly => b.min(s),
            Self::LightenOnly => b.max(s),
            Self::Divide => b / s.max(NEARLY_ZERO),
            Self::Dodge => b / (1.0 - s).max(NEARLY_ZERO),
            Self::Burn => 1.0 - (1.0 - b) / s.max(NEARLY_ZERO),
            Self::SoftLight => soft_light(b, s),
            Self::GrainExtract => b - s + 0.5,
            Self::GrainMerge => b + s - 0.5,
            Self::VividLight => vivid_light(b, s).clamp(0.0, 1.0),
            Self::PinLight => {
                if s < 0.5 {
                    b.min(2.0 * s)
                } else {
                    b.max(2.0 * s - 1.0)
                }
            }
            Self::LinearLight => b + 2.0 * s - 1.0,
            Self::HardMix => {
                if vivid_light(b, s) < 0.5 {
                    0.0
                } else {
                    1.0
                }
            }
            Self::Exclusion => b + s - 2.0 * b * s,
            Self::LinearBurn => b + s - 1.0,
            _ => s,
        }
    }
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s < 0.5 {
        2.0 * b * s
    } else {
        1.0 - 2.0 * (1.0 - b) * (1.0 - s)
    }
}

fn soft_light(b: f32, s: f32) -> f32 {
    let screen = 1.0 - (1.0 - b) * (1.0 - s);
    (1.0 - b) * b * s + b * screen
}

fn vivid_light(b: f32, s: f32) -> f32 {
    if s < 0.5 {
        1.0 - (1.0 - b) / (2.0 * s).max(NEARLY_ZERO)
    } else {
        b / (2.0 * (1.0 - s)).max(NEARLY_ZERO)
    }
}

fn luma(color: Rgb) -> f32 {
    color[0] * LUMA[0] + color[1] * LUMA[1] + color[2] * LUMA[2]
}

fn luminance_blend(backdrop: Rgb, source: Rgb) -> Rgb {
    let current = luma(backdrop);
    let wanted = luma(source);
    if current <= NEARLY_ZERO {
        return [wanted; 3];
    }
    let scale = wanted / current;
    [
        backdrop[0] * scale,
        backdrop[1] * scale,
        backdrop[2] * scale,
    ]
}

fn to_hsv(color: Rgb) -> Rgb {
    let max = color[0].max(color[1]).max(color[2]);
    let min = color[0].min(color[1]).min(color[2]);
    let delta = max - min;
    let value = max;
    let saturation = if max > 0.0 { delta / max } else { 0.0 };
    let hue = if delta <= 0.0 {
        0.0
    } else if (max - color[0]).abs() <= f32::EPSILON {
        ((color[1] - color[2]) / delta).rem_euclid(6.0) / 6.0
    } else if (max - color[1]).abs() <= f32::EPSILON {
        ((color[2] - color[0]) / delta + 2.0) / 6.0
    } else {
        ((color[0] - color[1]) / delta + 4.0) / 6.0
    };
    [hue, saturation, value]
}

fn from_hsv(hsv: Rgb) -> Rgb {
    let [hue, saturation, value] = hsv;
    let scaled = hue.rem_euclid(1.0) * 6.0;
    let fraction = scaled.fract();
    let low = value * (1.0 - saturation);
    let falling = value * (1.0 - saturation * fraction);
    let rising = value * (1.0 - saturation * (1.0 - fraction));
    if scaled < 1.0 {
        [value, rising, low]
    } else if scaled < 2.0 {
        [falling, value, low]
    } else if scaled < 3.0 {
        [low, value, rising]
    } else if scaled < 4.0 {
        [low, falling, value]
    } else if scaled < 5.0 {
        [rising, low, value]
    } else {
        [value, low, falling]
    }
}

fn hsv_blend(mode: Blend, backdrop: Rgb, source: Rgb) -> Rgb {
    let b = to_hsv(backdrop);
    let s = to_hsv(source);
    from_hsv(match mode {
        Blend::HsvHue => [s[0], b[1], b[2]],
        Blend::HsvSaturation => [b[0], s[1], b[2]],
        Blend::HsvColor => [s[0], s[1], b[2]],
        _ => [b[0], b[1], s[2]],
    })
}

fn multiply_matrix(matrix: &[[f32; 3]; 3], color: Rgb) -> Rgb {
    let row = |index: usize| {
        matrix[index][0] * color[0] + matrix[index][1] * color[1] + matrix[index][2] * color[2]
    };
    [row(0), row(1), row(2)]
}

fn lab_forward(value: f32) -> f32 {
    if value > LAB_EPSILON {
        value.cbrt()
    } else {
        (LAB_KAPPA * value + 16.0) / 116.0
    }
}

fn lab_backward(value: f32) -> f32 {
    let cubed = value * value * value;
    if cubed > LAB_EPSILON {
        cubed
    } else {
        (116.0 * value - 16.0) / LAB_KAPPA
    }
}

fn linear_to_lab(color: Rgb) -> Rgb {
    let xyz = multiply_matrix(&LINEAR_TO_XYZ, color);
    let x = lab_forward(xyz[0] / D50_WHITE[0]);
    let y = lab_forward(xyz[1] / D50_WHITE[1]);
    let z = lab_forward(xyz[2] / D50_WHITE[2]);
    [116.0 * y - 16.0, 500.0 * (x - y), 200.0 * (y - z)]
}

fn lab_to_linear(lab: Rgb) -> Rgb {
    let y = (lab[0] + 16.0) / 116.0;
    let x = y + lab[1] / 500.0;
    let z = y - lab[2] / 200.0;
    let xyz = [
        lab_backward(x) * D50_WHITE[0],
        lab_backward(y) * D50_WHITE[1],
        lab_backward(z) * D50_WHITE[2],
    ];
    multiply_matrix(&XYZ_TO_LINEAR, xyz)
}

fn lch_blend(mode: Blend, backdrop: Rgb, source: Rgb) -> Rgb {
    let b = polar(linear_to_lab(backdrop));
    let s = polar(linear_to_lab(source));
    let [lightness, chroma, hue] = match mode {
        Blend::LchHue => [b[0], b[1], s[2]],
        Blend::LchChroma => [b[0], s[1], b[2]],
        Blend::LchColor => [b[0], s[1], s[2]],
        _ => [s[0], b[1], b[2]],
    };
    lab_to_linear([lightness, chroma * hue.cos(), chroma * hue.sin()])
}

fn polar(lab: Rgb) -> Rgb {
    [lab[0], lab[1].hypot(lab[2]), lab[2].atan2(lab[1])]
}

const ENCODE_TABLE_STEPS: usize = 8192;

fn encode_table() -> &'static [f32] {
    static TABLE: OnceLock<Vec<f32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        (0..=ENCODE_TABLE_STEPS)
            .map(|step| {
                let value = f32::from(u16::try_from(step).unwrap_or(u16::MAX))
                    / f32::from(u16::try_from(ENCODE_TABLE_STEPS).unwrap_or(u16::MAX));
                encode_exactly(value)
            })
            .collect()
    })
}

fn encode_exactly(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(super) fn to_perceptual(value: f32) -> f32 {
    if !(0.0..1.0).contains(&value) {
        return encode_exactly(value);
    }
    let scaled = value * 8192.0;
    let step = scaled as usize;
    let fraction = scaled - scaled.floor();
    let table = encode_table();
    table[step] + (table[step + 1] - table[step]) * fraction
}

pub(super) fn to_linear(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn from_linear(space: Space, color: Rgb) -> Rgb {
    match space {
        Space::Linear => color,
        Space::Perceptual => color.map(to_perceptual),
    }
}

pub(super) fn to_linear_from(space: Space, color: Rgb) -> Rgb {
    match space {
        Space::Linear => color,
        Space::Perceptual => color.map(to_linear),
    }
}
