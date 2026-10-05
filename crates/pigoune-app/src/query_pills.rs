use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gettextrs::{gettext, pgettext};
use gtk::glib;
use pigoune_core::{query_groups, query_text};

type QueryChangedCallback = Rc<dyn Fn(String)>;

const MINIMUM_WORDS: usize = 2;

mod imp {
    use std::cell::RefCell;

    use adw::prelude::*;
    use adw::subclass::prelude::*;
    use gtk::glib;

    use super::QueryChangedCallback;

    #[derive(Default)]
    pub struct PigouneQueryPills {
        pub wrap: adw::WrapBox,
        pub groups: RefCell<Vec<Vec<String>>>,
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
            pills.set_child(Some(&self.wrap));
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

    pub fn show_query(&self, query: &str) {
        let groups = query_groups(query);
        if *self.imp().groups.borrow() == groups {
            return;
        }
        self.imp().groups.replace(groups);
        self.rebuild();
    }

    fn rebuild(&self) {
        let wrap = &self.imp().wrap;
        wrap.remove_all();
        let groups = self.imp().groups.borrow().clone();
        let words: usize = groups.iter().map(Vec::len).sum();
        self.set_visible(words >= MINIMUM_WORDS);
        if words < MINIMUM_WORDS {
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
