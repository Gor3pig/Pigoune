use super::{Point, Rect};

pub const SEED_OPTION: &str = "seed";
pub const ROUGHNESS_CHOICES: [&str; 3] = ["precise", "sketchy", "very_sketchy"];

const ELLIPSE_POINTS: u8 = 9;
const ELLIPSE_WANDER: f32 = 0.6;
const ELLIPSE_PASS_TIE: f32 = 0.85;
const ELLIPSE_OVERLAP: f32 = 0.2;
const QUARTER_ARC_HANDLE: f32 = 0.552_284_8;
const LONGEST_EFFECTIVE_LENGTH: f32 = 400.0;
const SHORTEST_VISIBLE_LENGTH: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Roughness {
    Precise,
    Sketchy,
    VerySketchy,
}

impl Roughness {
    #[must_use]
    pub fn from_choice(choice: &str) -> Self {
        match choice {
            "sketchy" => Self::Sketchy,
            "very_sketchy" => Self::VerySketchy,
            _ => Self::Precise,
        }
    }

    #[must_use]
    pub fn is_precise(self) -> bool {
        self == Self::Precise
    }

    fn parameters(self) -> Parameters {
        match self {
            Self::Precise => Parameters {
                bow: 0.0,
                jitter: 0.0,
                overshoot: 0.0,
                reach: 0.0,
            },
            Self::Sketchy => Parameters {
                bow: 0.18,
                jitter: 0.3,
                overshoot: 0.8,
                reach: 3.4,
            },
            Self::VerySketchy => Parameters {
                bow: 0.34,
                jitter: 0.5,
                overshoot: 1.2,
                reach: 6.0,
            },
        }
    }

    #[must_use]
    pub fn farthest_reach(self) -> f32 {
        let parameters = self.parameters();
        parameters.reach * (1.0 + parameters.jitter + parameters.overshoot)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hand {
    pub level: Roughness,
    pub seed: u32,
    pub passes: u8,
}

impl Hand {
    #[must_use]
    pub fn new(level: Roughness, seed: u32, broken_line: bool) -> Self {
        Self {
            level,
            seed,
            passes: if broken_line { 1 } else { 2 },
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Parameters {
    bow: f32,
    jitter: f32,
    overshoot: f32,
    reach: f32,
}

impl Parameters {
    fn amplitude(self, length: f32) -> f32 {
        (self.bow * length.clamp(0.0, LONGEST_EFFECTIVE_LENGTH).sqrt()).min(self.reach)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PathStep {
    MoveTo(Point),
    QuadTo(Point, Point),
    CubicTo(Point, Point, Point),
    LineTo(Point),
}

pub type Stroke = Vec<PathStep>;

struct Dice(u64);

impl Dice {
    fn new(seed: u32, salt: u32, pass: u8) -> Self {
        let mixed = (u64::from(seed) << 32) ^ (u64::from(salt) << 8) ^ u64::from(pass);
        let mut dice = Self(mixed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
        dice.next_bits();
        dice
    }

    fn next_bits(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "only the 24 high bits of the draw are kept"
    )]
    fn signed(&mut self) -> f32 {
        let unit = (self.next_bits() >> 40) as f32 / 16_777_216.0;
        unit.mul_add(2.0, -1.0)
    }

    fn unsigned(&mut self) -> f32 {
        f32::midpoint(self.signed(), 1.0)
    }
}

#[must_use]
pub fn seed_for_object(id: u32) -> u32 {
    let mut dice = Dice::new(id, 0x5eed, 0);
    u32::try_from(dice.next_bits() >> 32).unwrap_or(id)
}

fn offset(point: Point, along: (f32, f32), distance: f32) -> Point {
    Point {
        x: point.x + along.0 * distance,
        y: point.y + along.1 * distance,
    }
}

fn lerp(from: Point, to: Point, share: f32) -> Point {
    Point {
        x: from.x + (to.x - from.x) * share,
        y: from.y + (to.y - from.y) * share,
    }
}

fn direction(from: Point, to: Point) -> ((f32, f32), f32) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let length = dx.hypot(dy);
    if length < SHORTEST_VISIBLE_LENGTH {
        ((1.0, 0.0), 0.0)
    } else {
        ((dx / length, dy / length), length)
    }
}

fn wobbly_line(
    from: Point,
    to: Point,
    level: Roughness,
    dice: &mut Dice,
    overshoots: bool,
) -> Stroke {
    let parameters = level.parameters();
    let (along, length) = direction(from, to);
    let across = (-along.1, along.0);
    let amplitude = parameters.amplitude(length);
    let jitter = amplitude * parameters.jitter;
    let reach = amplitude * parameters.overshoot;
    let (start_gap, end_gap) = (dice.unsigned(), dice.unsigned());
    let (start_shift, end_shift) = (dice.signed(), dice.signed());
    let (first_bow, second_bow) = (dice.signed(), dice.signed());
    let (start_slide, end_slide) = if overshoots {
        (-start_gap * reach, end_gap * reach)
    } else {
        (0.0, 0.0)
    };
    let start = offset(
        offset(from, along, start_slide + jitter * start_shift),
        across,
        jitter * start_shift,
    );
    let end = offset(
        offset(to, along, end_slide + jitter * end_shift),
        across,
        jitter * end_shift,
    );
    let first = offset(lerp(start, end, 1.0 / 3.0), across, amplitude * first_bow);
    let second = offset(lerp(start, end, 2.0 / 3.0), across, amplitude * second_bow);
    vec![
        PathStep::MoveTo(start),
        PathStep::CubicTo(first, second, end),
    ]
}

#[must_use]
pub fn sketch_line(from: Point, to: Point, hand: Hand, salt: u32) -> Vec<Stroke> {
    (0..hand.passes)
        .map(|pass| {
            let mut dice = Dice::new(hand.seed, salt, pass);
            wobbly_line(from, to, hand.level, &mut dice, true)
        })
        .collect()
}

#[must_use]
pub fn sketch_outline(points: &[Point], closed: bool, hand: Hand, salt: u32) -> Vec<Stroke> {
    let edges = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    (0..edges)
        .map(|index| {
            let edge_salt = salt.wrapping_add(u32::try_from(index).unwrap_or(0));
            let mut dice = Dice::new(hand.seed, edge_salt, 0);
            let next = points[(index + 1) % points.len()];
            wobbly_line(points[index], next, hand.level, &mut dice, false)
        })
        .collect()
}

#[must_use]
pub fn sketched_curve(
    from: Point,
    control: Point,
    to: Point,
    hand: Hand,
    salt: u32,
    pass: u8,
) -> (Point, Point, Point) {
    let parameters = hand.level.parameters();
    let mut dice = Dice::new(hand.seed, salt, pass);
    let (along, length) = direction(from, to);
    let across = (-along.1, along.0);
    let amplitude = parameters.amplitude(length);
    let jitter = amplitude * parameters.jitter;
    let (start_shift, end_shift, bend) = (dice.signed(), dice.signed(), dice.signed());
    (
        offset(from, across, jitter * start_shift),
        offset(control, across, amplitude * bend),
        offset(to, across, jitter * end_shift),
    )
}

#[must_use]
pub fn sketch_curve(from: Point, control: Point, to: Point, hand: Hand, salt: u32) -> Vec<Stroke> {
    (0..hand.passes)
        .map(|pass| {
            let (start, bend, end) = sketched_curve(from, control, to, hand, salt, pass);
            vec![PathStep::MoveTo(start), PathStep::QuadTo(bend, end)]
        })
        .collect()
}

fn smooth_through(points: &[Point]) -> Stroke {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let mut steps = vec![PathStep::MoveTo(*first)];
    let last = points.len() - 1;
    for index in 0..last {
        let before = points[index.saturating_sub(1)];
        let (from, to) = (points[index], points[index + 1]);
        let after = points[(index + 2).min(last)];
        steps.push(PathStep::CubicTo(
            Point {
                x: from.x + (to.x - before.x) / 6.0,
                y: from.y + (to.y - before.y) / 6.0,
            },
            Point {
                x: to.x - (after.x - from.x) / 6.0,
                y: to.y - (after.y - from.y) / 6.0,
            },
            to,
        ));
    }
    steps
}

#[must_use]
pub fn sketch_ellipse(bounds: Rect, hand: Hand, salt: u32) -> Vec<Stroke> {
    let (radius_x, radius_y) = (bounds.width / 2.0, bounds.height / 2.0);
    let center = Point {
        x: bounds.x + radius_x,
        y: bounds.y + radius_y,
    };
    let reach = hand
        .level
        .parameters()
        .amplitude((radius_x + radius_y).max(0.0));
    let amplitude = reach * ELLIPSE_WANDER;
    let mut first_dice = Dice::new(hand.seed, salt, 0);
    first_dice.signed();
    first_dice.unsigned();
    let shared: Vec<f32> = (0..=ELLIPSE_POINTS).map(|_| first_dice.signed()).collect();
    (0..hand.passes)
        .map(|pass| {
            let mut dice = Dice::new(hand.seed, salt, pass);
            let start = dice.signed() * std::f32::consts::PI;
            let sweep = std::f32::consts::TAU + ELLIPSE_OVERLAP * (1.0 + dice.unsigned());
            let count = f32::from(ELLIPSE_POINTS);
            let points: Vec<Point> = (0..=ELLIPSE_POINTS)
                .map(|step| {
                    let angle = start + sweep * f32::from(step) / count;
                    let fresh = dice.signed();
                    let wander = amplitude
                        * if pass == 0 {
                            fresh
                        } else {
                            shared[usize::from(step)] * ELLIPSE_PASS_TIE
                                + fresh * (1.0 - ELLIPSE_PASS_TIE)
                        };
                    let (sine, cosine) = angle.sin_cos();
                    Point {
                        x: center.x + (radius_x + wander) * cosine,
                        y: center.y + (radius_y + wander) * sine,
                    }
                })
                .collect();
            smooth_through(&points)
        })
        .collect()
}

fn corner_arcs(bounds: Rect, corner: f32) -> [[Point; 4]; 4] {
    let (left, top) = (bounds.x, bounds.y);
    let (right, bottom) = (left + bounds.width, top + bounds.height);
    let handle = corner * (1.0 - QUARTER_ARC_HANDLE);
    let at = |x: f32, y: f32| Point { x, y };
    [
        [
            at(right - corner, top),
            at(right - handle, top),
            at(right, top + handle),
            at(right, top + corner),
        ],
        [
            at(right, bottom - corner),
            at(right, bottom - handle),
            at(right - handle, bottom),
            at(right - corner, bottom),
        ],
        [
            at(left + corner, bottom),
            at(left + handle, bottom),
            at(left, bottom - handle),
            at(left, bottom - corner),
        ],
        [
            at(left, top + corner),
            at(left, top + handle),
            at(left + handle, top),
            at(left + corner, top),
        ],
    ]
}

#[must_use]
pub fn sketch_rectangle(bounds: Rect, corner: f32, hand: Hand, salt: u32) -> Vec<Stroke> {
    let corner = corner.clamp(0.0, bounds.width.min(bounds.height) / 2.0);
    let (left, top) = (bounds.x, bounds.y);
    let (right, bottom) = (left + bounds.width, top + bounds.height);
    let at = |x: f32, y: f32| Point { x, y };
    let sides = [
        (at(left + corner, top), at(right - corner, top)),
        (at(right, top + corner), at(right, bottom - corner)),
        (at(right - corner, bottom), at(left + corner, bottom)),
        (at(left, bottom - corner), at(left, top + corner)),
    ];
    let mut strokes = Vec::new();
    for (index, (from, to)) in sides.iter().enumerate() {
        let side_salt = salt.wrapping_add(u32::try_from(index).unwrap_or(0));
        strokes.extend(sketch_line(*from, *to, hand, side_salt));
    }
    if corner > 0.0 {
        let parameters = hand.level.parameters();
        let amplitude = parameters.amplitude(corner * 4.0) * parameters.jitter;
        for (index, arc) in corner_arcs(bounds, corner).iter().enumerate() {
            let arc_salt = salt.wrapping_add(8 + u32::try_from(index).unwrap_or(0));
            for pass in 0..hand.passes {
                let mut dice = Dice::new(hand.seed, arc_salt, pass);
                let [start, first, second, end] = arc.map(|point| Point {
                    x: point.x + amplitude * dice.signed(),
                    y: point.y + amplitude * dice.signed(),
                });
                strokes.push(vec![
                    PathStep::MoveTo(start),
                    PathStep::CubicTo(first, second, end),
                ]);
            }
        }
    }
    strokes
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEVELS: [Roughness; 2] = [Roughness::Sketchy, Roughness::VerySketchy];

    fn point(x: f32, y: f32) -> Point {
        Point { x, y }
    }

    fn every_point(strokes: &[Stroke]) -> Vec<Point> {
        strokes
            .iter()
            .flatten()
            .flat_map(|step| match *step {
                PathStep::MoveTo(a) | PathStep::LineTo(a) => vec![a],
                PathStep::QuadTo(a, b) => vec![a, b],
                PathStep::CubicTo(a, b, c) => vec![a, b, c],
            })
            .collect()
    }

    fn drawn_points(strokes: &[Stroke]) -> Vec<Point> {
        let mut drawn = Vec::new();
        for stroke in strokes {
            let mut current = point(0.0, 0.0);
            for step in stroke {
                match *step {
                    PathStep::MoveTo(to) | PathStep::LineTo(to) => {
                        current = to;
                        drawn.push(to);
                    }
                    PathStep::QuadTo(control, to) => {
                        for index in 1..=10_u8 {
                            let t = f32::from(index) / 10.0;
                            let u = 1.0 - t;
                            drawn.push(point(
                                u * u * current.x + 2.0 * u * t * control.x + t * t * to.x,
                                u * u * current.y + 2.0 * u * t * control.y + t * t * to.y,
                            ));
                        }
                        current = to;
                    }
                    PathStep::CubicTo(first, second, to) => {
                        for index in 1..=10_u8 {
                            let t = f32::from(index) / 10.0;
                            let u = 1.0 - t;
                            drawn.push(point(
                                u * u * u * current.x
                                    + 3.0 * u * u * t * first.x
                                    + 3.0 * u * t * t * second.x
                                    + t * t * t * to.x,
                                u * u * u * current.y
                                    + 3.0 * u * u * t * first.y
                                    + 3.0 * u * t * t * second.y
                                    + t * t * t * to.y,
                            ));
                        }
                        current = to;
                    }
                }
            }
        }
        drawn
    }

    fn hand(level: Roughness, seed: u32) -> Hand {
        Hand::new(level, seed, false)
    }

    fn area(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_same_seed_always_draws_the_same_strokes() {
        for level in LEVELS {
            let draw = || sketch_line(point(0.0, 0.0), point(120.0, 40.0), hand(level, 77), 1);
            assert_eq!(draw(), draw());
        }
    }

    #[test]
    fn another_seed_or_salt_draws_other_strokes() {
        let line = |seed, salt| {
            let hand = hand(Roughness::Sketchy, seed);
            sketch_line(point(0.0, 0.0), point(120.0, 40.0), hand, salt)
        };
        assert_ne!(line(1, 1), line(2, 1));
        assert_ne!(line(1, 1), line(1, 2));
    }

    #[test]
    fn every_pass_of_a_line_differs_so_that_it_looks_hand_drawn() {
        let hand = hand(Roughness::VerySketchy, 5);
        let strokes = sketch_line(point(0.0, 0.0), point(200.0, 0.0), hand, 0);
        assert_eq!(strokes.len(), 2);
        assert_ne!(strokes[0], strokes[1]);
    }

    #[test]
    fn a_broken_line_is_drawn_once_so_that_its_dashes_do_not_double_up() {
        let hand = Hand::new(Roughness::Sketchy, 5, true);
        assert_eq!(
            sketch_line(point(0.0, 0.0), point(200.0, 0.0), hand, 0).len(),
            1
        );
        let two_passes = Hand::new(Roughness::Sketchy, 5, false);
        let once = sketch_line(point(0.0, 0.0), point(200.0, 0.0), hand, 0);
        let twice = sketch_line(point(0.0, 0.0), point(200.0, 0.0), two_passes, 0);
        assert_eq!(once[0], twice[0]);
        assert_eq!(sketch_ellipse(area(0.0, 0.0, 80.0, 40.0), hand, 0).len(), 1);
        assert_eq!(
            sketch_rectangle(area(0.0, 0.0, 80.0, 40.0), 8.0, hand, 0).len(),
            8
        );
    }

    #[test]
    fn the_stronger_level_wanders_farther_but_never_beyond_its_reach() {
        let mut widest_of = Vec::new();
        for level in LEVELS {
            let mut widest = 0.0_f32;
            for seed in 0..200 {
                let strokes = sketch_line(point(0.0, 0.0), point(400.0, 0.0), hand(level, seed), 0);
                for point in every_point(&strokes) {
                    widest = widest.max(point.y.abs());
                }
            }
            assert!(
                widest <= level.farthest_reach(),
                "{level:?} wandered {widest}"
            );
            widest_of.push(widest);
        }
        assert!(widest_of[0] > 0.5, "the subtle level is too tame");
        assert!(
            widest_of[1] > widest_of[0] * 1.5,
            "the strong level is not stronger"
        );
    }

    #[test]
    fn nothing_wanders_farther_than_the_reach_around_a_rectangle_or_an_ellipse() {
        for level in LEVELS {
            let reach = level.farthest_reach();
            for seed in 0..100 {
                let hand = hand(level, seed);
                let rectangle = sketch_rectangle(area(10.0, 20.0, 300.0, 200.0), 24.0, hand, 3);
                let ellipse = sketch_ellipse(area(10.0, 20.0, 300.0, 200.0), hand, 4);
                for set in [rectangle, ellipse] {
                    for point in drawn_points(&set) {
                        assert!(point.x >= 10.0 - reach && point.x <= 310.0 + reach);
                        assert!(point.y >= 20.0 - reach && point.y <= 220.0 + reach);
                    }
                }
            }
        }
    }

    #[test]
    fn a_slightly_larger_shape_keeps_the_same_hand_instead_of_boiling() {
        let hand = hand(Roughness::Sketchy, 9);
        let small = every_point(&sketch_line(point(0.0, 0.0), point(100.0, 0.0), hand, 0));
        let large = every_point(&sketch_line(point(0.0, 0.0), point(104.0, 0.0), hand, 0));
        assert_eq!(small.len(), large.len());
        for (a, b) in small.iter().zip(&large) {
            assert!((a.y - b.y).abs() < 0.5, "{a:?} {b:?}");
        }
    }

    #[test]
    fn an_empty_or_degenerate_shape_does_not_produce_invalid_numbers() {
        for level in LEVELS {
            let hand = hand(level, 1);
            let strokes = [
                sketch_line(point(5.0, 5.0), point(5.0, 5.0), hand, 0),
                sketch_rectangle(area(0.0, 0.0, 0.0, 0.0), 10.0, hand, 0),
                sketch_ellipse(area(0.0, 0.0, 0.0, 0.0), hand, 0),
                sketch_outline(&[point(1.0, 1.0)], true, hand, 0),
                sketch_outline(&[], false, hand, 0),
                sketch_curve(point(0.0, 0.0), point(0.0, 0.0), point(0.0, 0.0), hand, 0),
            ];
            for set in strokes {
                for point in every_point(&set) {
                    assert!(point.x.is_finite() && point.y.is_finite());
                }
            }
        }
    }

    #[test]
    fn the_seed_of_an_object_is_stable_and_differs_between_objects() {
        assert_eq!(seed_for_object(3), seed_for_object(3));
        assert_ne!(seed_for_object(3), seed_for_object(4));
    }

    #[test]
    fn the_hand_drawn_strokes_of_a_fixed_seed_never_change_silently() {
        let hand = hand(Roughness::Sketchy, 42);
        let strokes = sketch_line(point(0.0, 0.0), point(100.0, 50.0), hand, 7);
        let text: Vec<String> = every_point(&strokes)
            .iter()
            .map(|point| format!("{:.2},{:.2}", point.x, point.y))
            .collect();
        assert_eq!(
            text.join(" "),
            "-1.02,-0.17 32.91,16.74 66.26,34.83 100.75,50.62 -0.56,-0.03 32.74,17.96 66.50,35.02 101.04,50.52"
        );
    }

    #[test]
    fn choices_that_are_unknown_read_as_precise() {
        assert_eq!(Roughness::from_choice("sketchy"), Roughness::Sketchy);
        assert_eq!(
            Roughness::from_choice("very_sketchy"),
            Roughness::VerySketchy
        );
        assert_eq!(Roughness::from_choice("wild"), Roughness::Precise);
        assert!(Roughness::from_choice("").is_precise());
    }
}
