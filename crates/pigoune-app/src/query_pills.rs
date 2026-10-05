use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, ngettext, pgettext};
use gtk::glib;
use pigoune_core::{MAX_QUERY_WORDS, query_groups, query_text, query_word_count};

type QueryChangedCallback = Rc<dyn Fn(String)>;

const MINIMUM_WORDS: usize = 2;

mod imp {
    use std::cell::{Cell, RefCell};

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::QueryChangedCallback;

    #[derive(Default)]
    pub struct PigouneQueryPills {
        pub wrap: adw::WrapBox,
        pub count_label: gtk::Label,
        pub groups: RefCell<Vec<Vec<String>>>,
        pub result_count: Cell<Option<usize>>,
        pub truncated: Cell<bool>,
        pub on_query_changed: RefCell<Option<QueryChangedCallback>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PigouneQueryPills {
        const NAME: &'static str = "PigouneQueryPills";
        type Type = super::PigouneQueryPills;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for PigouneQueryPills {
        fn constructed(&self) {
            self.parent_constructed();
            let pills = self.obj();
            self.wrap.set_child_spacing(6);
            self.wrap.set_line_spacing(6);
            self.wrap.set_hexpand(true);
            self.count_label.add_css_class("dim-label");
            self.count_label.add_css_class("numeric");
            self.count_label.set_valign(gtk::Align::Center);
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            row.append(&self.wrap);
            row.append(&self.count_label);
            pills.set_child(Some(&row));
            pills.add_css_class("query-pills");
            pills.set_visible(false);
        }
    }

    impl WidgetImpl for PigouneQueryPills {}
    impl BinImpl for PigouneQueryPills {}
}

glib::wrapper! {
    pub struct PigouneQueryPills(ObjectSubclass<imp::PigouneQueryPills>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl PigouneQueryPills {
    pub fn connect_query_changed(&self, callback: impl Fn(String) + 'static) {
        self.imp().on_query_changed.replace(Some(Rc::new(callback)));
    }

    pub fn show_result_count(&self, count: Option<usize>, place: &str) {
        let imp = self.imp();
        imp.result_count.set(count);
        imp.count_label.set_visible(count.is_some());
        if let Some(count) = count {
            imp.count_label.set_label(
                &ngettext(
                    "{count} result in {place}",
                    "{count} results in {place}",
                    u32::try_from(count).unwrap_or(u32::MAX),
                )
                .replace("{count}", &count.to_string())
                .replace("{place}", place),
            );
        }
        self.update_visibility();
    }

    fn word_count(&self) -> usize {
        self.imp().groups.borrow().iter().map(Vec::len).sum()
    }

    fn update_visibility(&self) {
        let shows_words = self.word_count() >= MINIMUM_WORDS;
        self.set_visible(shows_words || self.imp().result_count.get().is_some());
    }

    pub fn show_query(&self, query: &str) {
        let imp = self.imp();
        let groups = query_groups(query);
        let truncated = query_word_count(query) > MAX_QUERY_WORDS;
        if *imp.groups.borrow() == groups && imp.truncated.get() == truncated {
            return;
        }
        imp.groups.replace(groups);
        imp.truncated.set(truncated);
        self.rebuild();
    }

    fn rebuild(&self) {
        let wrap = &self.imp().wrap;
        wrap.remove_all();
        let groups = self.imp().groups.borrow().clone();
        self.update_visibility();
        if self.word_count() < MINIMUM_WORDS {
            return;
        }
        for (group_index, group) in groups.iter().enumerate() {
            for (word_index, word) in group.iter().enumerate() {
                let place = WordPlace {
                    group: group_index,
                    word: word_index,
                };
                wrap.append(&self.word_pill(word, place));
                let last_group = group_index + 1 == groups.len();
                let last_word = word_index + 1 == group.len();
                if !(last_group && last_word) {
                    wrap.append(&self.connector(place, last_word));
                }
            }
        }
        if self.imp().truncated.get() {
            wrap.append(
                &gtk::Label::builder()
                    .label(
                        gettext("{max} words at most: the next ones are ignored")
                            .replace("{max}", &MAX_QUERY_WORDS.to_string()),
                    )
                    .css_classes(["dim-label", "caption"])
                    .valign(gtk::Align::Center)
                    .build(),
            );
        }
    }

    fn word_pill(&self, word: &str, place: WordPlace) -> gtk::Widget {
        let pill = gtk::Box::builder()
            .spacing(2)
            .css_classes(["query-pill"])
            .build();
        pill.append(&gtk::Label::new(Some(word)));
        let remove = gtk::Button::builder()
            .icon_name("window-close-symbolic")
            .tooltip_text(gettext("Remove “{word}” From the Search").replace("{word}", word))
            .css_classes(["flat", "circular", "query-pill-remove"])
            .valign(gtk::Align::Center)
            .build();
        remove.connect_clicked(glib::clone!(
            #[weak(rename_to = pills)]
            self,
            move |_| pills.change(|groups| without_word(groups, place))
        ));
        pill.append(&remove);
        pill.upcast()
    }

    fn connector(&self, place: WordPlace, alternative: bool) -> gtk::Widget {
        let (label, tooltip, class) = if alternative {
            (
                pgettext("search connector", "or"),
                gettext("Require Both Words"),
                "query-or",
            )
        } else {
            (
                pgettext("search connector", "and"),
                gettext("Accept Either Word"),
                "query-and",
            )
        };
        let button = gtk::Button::builder()
            .label(label)
            .tooltip_text(tooltip)
            .css_classes(["flat", "query-connector", class])
            .valign(gtk::Align::Center)
            .build();
        button.connect_clicked(glib::clone!(
            #[weak(rename_to = pills)]
            self,
            move |_| {
                pills.change(|groups| {
                    if alternative {
                        joined_with_next(groups, place.group)
                    } else {
                        split_after(groups, place)
                    }
                });
            }
        ));
        button.upcast()
    }

    fn change(&self, change: impl FnOnce(&[Vec<String>]) -> Vec<Vec<String>>) {
        let changed = change(&self.imp().groups.borrow());
        self.imp().groups.replace(changed.clone());
        self.imp().truncated.set(false);
        let callback = self.imp().on_query_changed.borrow().clone();
        if let Some(callback) = callback {
            callback(query_text(&changed));
        }
        let pills = self.downgrade();
        glib::idle_add_local_once(move || {
            if let Some(pills) = pills.upgrade() {
                pills.rebuild();
            }
        });
    }
}

impl Default for PigouneQueryPills {
    fn default() -> Self {
        glib::Object::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WordPlace {
    group: usize,
    word: usize,
}

fn without_word(groups: &[Vec<String>], place: WordPlace) -> Vec<Vec<String>> {
    groups
        .iter()
        .enumerate()
        .map(|(group_index, words)| {
            words
                .iter()
                .enumerate()
                .filter(|(word_index, _)| (group_index, *word_index) != (place.group, place.word))
                .map(|(_, word)| word.clone())
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
        .collect()
}

fn split_after(groups: &[Vec<String>], place: WordPlace) -> Vec<Vec<String>> {
    let mut changed = Vec::with_capacity(groups.len() + 1);
    for (group_index, words) in groups.iter().enumerate() {
        if group_index == place.group && place.word + 1 < words.len() {
            changed.push(words[..=place.word].to_vec());
            changed.push(words[place.word + 1..].to_vec());
        } else {
            changed.push(words.clone());
        }
    }
    changed
}

fn joined_with_next(groups: &[Vec<String>], group: usize) -> Vec<Vec<String>> {
    let mut changed = Vec::with_capacity(groups.len());
    for (group_index, words) in groups.iter().enumerate() {
        if group_index == group + 1
            && let Some(previous) = changed.last_mut()
        {
            let previous: &mut Vec<String> = previous;
            previous.extend(words.iter().cloned());
            continue;
        }
        changed.push(words.clone());
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::{WordPlace, joined_with_next, split_after, without_word};

    fn groups(text: &[&[&str]]) -> Vec<Vec<String>> {
        text.iter()
            .map(|words| words.iter().map(|word| (*word).to_owned()).collect())
            .collect()
    }

    #[test]
    fn an_and_becomes_an_or_by_splitting_the_group() {
        let query = groups(&[&["logo", "rouge", "plat"], &["chèvre"]]);
        assert_eq!(
            split_after(&query, WordPlace { group: 0, word: 0 }),
            groups(&[&["logo"], &["rouge", "plat"], &["chèvre"]])
        );
    }

    #[test]
    fn an_or_becomes_an_and_by_joining_the_groups() {
        let query = groups(&[&["logo"], &["rouge"], &["chèvre"]]);
        assert_eq!(
            joined_with_next(&query, 1),
            groups(&[&["logo"], &["rouge", "chèvre"]])
        );
    }

    #[test]
    fn removing_a_word_drops_its_group_when_empty() {
        let query = groups(&[&["logo", "rouge"], &["chèvre"]]);
        assert_eq!(
            without_word(&query, WordPlace { group: 1, word: 0 }),
            groups(&[&["logo", "rouge"]])
        );
        assert_eq!(
            without_word(&query, WordPlace { group: 0, word: 1 }),
            groups(&[&["logo"], &["chèvre"]])
        );
    }
}
