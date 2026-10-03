use adw::subclass::prelude::*;
use gettextrs::ngettext;
use gtk::glib;
use gtk::prelude::*;

const SPACING: i32 = 4;
const MOST_LINES: usize = 2;

#[derive(Debug)]
struct Placement {
    shown: Vec<(i32, usize)>,
    overflow: Option<(i32, usize)>,
    add: (i32, usize),
}

mod imp {
    use std::cell::{Cell, OnceCell, RefCell};

    use adw::subclass::prelude::*;
    use gtk::glib;
    use gtk::prelude::*;

    #[derive(Default)]
    pub struct PigouneTagCloud {
        pub pills: RefCell<Vec<gtk::Widget>>,
        pub overflow: OnceCell<gtk::Button>,
        pub add: OnceCell<gtk::Button>,
        pub hidden: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneTagCloud {
        const NAME: &'static str = "PigouneTagCloud";
        type Type = super::PigouneTagCloud;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for PigouneTagCloud {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().build();
        }

        fn dispose(&self) {
            for pill in self.pills.take() {
                pill.unparent();
            }
            if let Some(overflow) = self.overflow.get() {
                overflow.unparent();
            }
            if let Some(add) = self.add.get() {
                add.unparent();
            }
        }
    }

    impl WidgetImpl for PigouneTagCloud {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let cloud = self.obj();
            if orientation == gtk::Orientation::Horizontal {
                let (minimum, natural) = cloud.widths();
                return (minimum, natural, -1, -1);
            }
            let width = if for_size < 0 {
                cloud.widths().1
            } else {
                for_size
            };
            let lines = cloud.placement(width).lines();
            let row = cloud.row_height();
            let height = lines * row + (lines - 1).max(0) * super::SPACING;
            (height, height, -1, -1)
        }

        fn size_allocate(&self, width: i32, _height: i32, _baseline: i32) {
            self.obj().place_children(width);
        }
    }
}

glib::wrapper! {
    pub struct PigouneTagCloud(ObjectSubclass<imp::PigouneTagCloud>)
        @extends gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl Placement {
    fn lines(&self) -> i32 {
        let last_line = self
            .shown
            .iter()
            .map(|(_, line)| *line)
            .chain(self.overflow.map(|(_, line)| line))
            .chain(std::iter::once(self.add.1))
            .max()
            .unwrap_or(0);
        i32::try_from(last_line + 1).unwrap_or(1)
    }
}

impl PigouneTagCloud {
    pub fn add_button(&self) -> gtk::Button {
        part(&self.imp().add)
    }

    pub fn overflow_button(&self) -> gtk::Button {
        part(&self.imp().overflow)
    }

    pub fn set_pills(&self, pills: Vec<gtk::Widget>) {
        let imp = self.imp();
        for pill in imp.pills.take() {
            pill.unparent();
        }
        let overflow = part(&imp.overflow);
        for pill in &pills {
            pill.insert_before(self, Some(&overflow));
        }
        imp.pills.replace(pills);
        self.queue_resize();
    }

    fn build(&self) {
        let imp = self.imp();
        let overflow = gtk::Button::builder()
            .label("+0")
            .visible(false)
            .css_classes(["flat", "tag-pill", "tag-overflow"])
            .build();
        let add = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .valign(gtk::Align::Center)
            .css_classes(["flat", "circular", "tag-add"])
            .build();
        overflow.set_parent(self);
        add.set_parent(self);
        set_part(&imp.overflow, overflow);
        set_part(&imp.add, add);
    }

    fn widths(&self) -> (i32, i32) {
        let add = natural_width(&part(&self.imp().add).upcast());
        let natural = self
            .imp()
            .pills
            .borrow()
            .iter()
            .map(natural_width)
            .fold(add, |total, width| total + width + SPACING);
        (add, natural)
    }

    fn row_height(&self) -> i32 {
        let imp = self.imp();
        imp.pills
            .borrow()
            .iter()
            .chain(std::iter::once(&part(&imp.add).upcast::<gtk::Widget>()))
            .map(|child| child.measure(gtk::Orientation::Vertical, -1).1)
            .max()
            .unwrap_or(0)
    }

    fn placement(&self, width: i32) -> Placement {
        let imp = self.imp();
        let pills: Vec<i32> = imp.pills.borrow().iter().map(natural_width).collect();
        let add = natural_width(&part(&imp.add).upcast());
        let overflow = natural_width(&part(&imp.overflow).upcast());
        place(&pills, add, overflow, width)
    }

    fn place_children(&self, width: i32) {
        let imp = self.imp();
        let placement = self.placement(width);
        let row = self.row_height();
        let top_of = |line: usize| i32::try_from(line).unwrap_or(0) * (row + SPACING);
        let pills = imp.pills.borrow();
        for (index, pill) in pills.iter().enumerate() {
            match placement.shown.get(index) {
                Some(&(x, line)) => {
                    pill.set_child_visible(true);
                    allocate(pill, x, top_of(line), row);
                }
                None => pill.set_child_visible(false),
            }
        }
        let overflow = part(&imp.overflow);
        match placement.overflow {
            Some((x, line)) => {
                overflow.set_visible(true);
                allocate(&overflow.clone().upcast(), x, top_of(line), row);
            }
            None => overflow.set_visible(false),
        }
        let (x, line) = placement.add;
        allocate(&part(&imp.add).upcast(), x, top_of(line), row);
        self.announce_hidden(pills.len() - placement.shown.len());
    }

    fn announce_hidden(&self, hidden: usize) {
        let imp = self.imp();
        if imp.hidden.replace(hidden) == hidden {
            return;
        }
        let overflow = part(&imp.overflow);
        glib::idle_add_local_once(move || {
            overflow.set_label(&format!("+{hidden}"));
            overflow.set_tooltip_text(Some(
                &ngettext(
                    "{count} more tag",
                    "{count} more tags",
                    u32::try_from(hidden).unwrap_or(u32::MAX),
                )
                .replace("{count}", &hidden.to_string()),
            ));
        });
    }
}

fn place(pills: &[i32], add: i32, overflow: i32, width: i32) -> Placement {
    let mut everything = Placement {
        shown: Vec::new(),
        overflow: None,
        add: (0, 0),
    };
    let (mut x, mut line) = (0, 0);
    for &pill in pills {
        if x > 0 && x + pill > width {
            line += 1;
            x = 0;
        }
        everything.shown.push((x, line));
        x += pill + SPACING;
    }
    if x > 0 && x + add > width {
        line += 1;
        x = 0;
    }
    everything.add = (x, line);
    if line < MOST_LINES {
        return everything;
    }

    let mut cut = Placement {
        shown: Vec::new(),
        overflow: None,
        add: (0, 0),
    };
    let tail = overflow + SPACING + add;
    let (mut x, mut line) = (0, 0);
    for &pill in pills {
        let (next_x, next_line) = if x > 0 && x + pill > width {
            (0, line + 1)
        } else {
            (x, line)
        };
        let on_last_line = next_line + 1 >= MOST_LINES;
        if next_line >= MOST_LINES || on_last_line && next_x + pill + SPACING + tail > width {
            break;
        }
        cut.shown.push((next_x, next_line));
        x = next_x + pill + SPACING;
        line = next_line;
    }
    if line + 1 < MOST_LINES && x > 0 && x + tail > width {
        line += 1;
        x = 0;
    }
    cut.overflow = Some((x, line));
    cut.add = (x + overflow + SPACING, line);
    cut
}

fn natural_width(widget: &gtk::Widget) -> i32 {
    widget.measure(gtk::Orientation::Horizontal, -1).1
}

fn allocate(widget: &gtk::Widget, x: i32, y: i32, height: i32) {
    let width = natural_width(widget);
    widget.size_allocate(&gtk::Allocation::new(x, y, width, height), -1);
}

impl Default for PigouneTagCloud {
    fn default() -> Self {
        glib::Object::new()
    }
}

fn part<Widget: Clone>(cell: &std::cell::OnceCell<Widget>) -> Widget {
    cell.get()
        .cloned()
        .expect("the tag cloud builds its parts at construction")
}

fn set_part<Widget>(cell: &std::cell::OnceCell<Widget>, widget: Widget) {
    if cell.set(widget).is_err() {
        unreachable!("the tag cloud builds its parts once");
    }
}

#[cfg(test)]
mod tests {
    use super::{MOST_LINES, place};

    #[test]
    fn everything_fits_on_one_line_when_there_is_room() {
        let placement = place(&[40, 40, 40], 24, 30, 400);
        assert_eq!(placement.shown, [(0, 0), (44, 0), (88, 0)]);
        assert_eq!(placement.overflow, None);
        assert_eq!(placement.add, (132, 0));
    }

    #[test]
    fn pills_wrap_onto_a_second_line() {
        let placement = place(&[60, 60, 60], 24, 30, 140);
        assert_eq!(placement.shown, [(0, 0), (64, 0), (0, 1)]);
        assert_eq!(placement.overflow, None);
        assert_eq!(placement.add, (64, 1));
    }

    #[test]
    fn extra_pills_are_counted_instead_of_adding_a_third_line() {
        let pills = [60; 10];
        let placement = place(&pills, 24, 30, 140);
        assert!(placement.shown.iter().all(|(_, line)| *line < MOST_LINES));
        assert_eq!(placement.shown.len(), 3);
        let (overflow_x, overflow_line) = placement.overflow.expect("an overflow count");
        assert_eq!(overflow_line, MOST_LINES - 1);
        assert!(overflow_x + 30 + 4 + 24 <= 140);
        assert_eq!(placement.add.1, MOST_LINES - 1);
    }

    #[test]
    fn the_count_never_covers_a_pill_when_the_cut_falls_on_a_new_line() {
        let pills = [100, 100, 100, 100];
        let placement = place(&pills, 24, 30, 160);
        let (overflow_x, overflow_line) = placement.overflow.expect("an overflow count");
        for (x, line) in &placement.shown {
            if *line == overflow_line {
                assert!(x + 100 + 4 <= overflow_x, "{placement:?}");
            }
        }
        assert!(overflow_line < MOST_LINES);
    }

    #[test]
    fn an_empty_cloud_only_shows_the_add_button() {
        let placement = place(&[], 24, 30, 140);
        assert!(placement.shown.is_empty());
        assert_eq!(placement.overflow, None);
        assert_eq!(placement.add, (0, 0));
    }
}
