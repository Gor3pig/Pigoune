use pigoune_core::{Tag, TagId};

use crate::sidebar_tag_cloud::TagPill;

pub trait TreeNode {
    fn node_id(&self) -> TagId;
    fn node_name(&self) -> &str;
    fn parent_id(&self) -> Option<TagId>;
}

impl TreeNode for Tag {
    fn node_id(&self) -> TagId {
        self.id
    }

    fn node_name(&self) -> &str {
        &self.name
    }

    fn parent_id(&self) -> Option<TagId> {
        self.parent
    }
}

impl TreeNode for TagPill {
    fn node_id(&self) -> TagId {
        self.id
    }

    fn node_name(&self) -> &str {
        &self.name
    }

    fn parent_id(&self) -> Option<TagId> {
        self.parent
    }
}

pub fn find<T: TreeNode>(all: &[T], id: TagId) -> Option<&T> {
    all.iter().find(|node| node.node_id() == id)
}

pub fn ancestors<'a, T: TreeNode>(all: &'a [T], node: &T) -> Vec<&'a T> {
    let mut found = Vec::new();
    let mut next = node.parent_id();
    while let Some(id) = next {
        let Some(parent) = find(all, id).filter(|_| found.len() < all.len()) else {
            break;
        };
        found.push(parent);
        next = parent.parent_id();
    }
    found.reverse();
    found
}

pub fn branch<T: TreeNode>(all: &[T], id: TagId) -> Vec<&T> {
    let Some(node) = find(all, id) else {
        return Vec::new();
    };
    let mut found = ancestors(all, node);
    found.push(node);
    found
}

pub fn path_text<T: TreeNode>(all: &[T], id: TagId) -> String {
    branch(all, id)
        .iter()
        .map(|node| node.node_name())
        .collect::<Vec<_>>()
        .join(" › ")
}

pub fn descendants<T: TreeNode>(all: &[T], root: TagId) -> Vec<TagId> {
    let mut found = vec![root];
    let mut index = 0;
    while index < found.len() {
        let parent = found[index];
        found.extend(
            all.iter()
                .filter(|node| node.parent_id() == Some(parent))
                .map(TreeNode::node_id),
        );
        index += 1;
    }
    found
}

pub fn sub_tag_count<T: TreeNode>(all: &[T], root: TagId) -> usize {
    descendants(all, root).len() - 1
}

pub fn is_within<T: TreeNode>(all: &[T], id: TagId, root: TagId) -> bool {
    branch(all, id).iter().any(|node| node.node_id() == root)
}

#[cfg(test)]
mod tests {
    use pigoune_core::{Tag, TagId};

    use super::{ancestors, branch, descendants, is_within, path_text, sub_tag_count};

    fn tag(number: u8, name: &str, parent: Option<&Tag>) -> Tag {
        Tag {
            id: TagId::parse(&format!("00000000-0000-7000-8000-{number:012}")).expect("id"),
            name: name.to_owned(),
            parent: parent.map(|parent| parent.id),
        }
    }

    #[test]
    fn sub_tags_are_counted_at_every_level() {
        let subject = tag(1, "subject", None);
        let animals = tag(2, "animals", Some(&subject));
        let goat = tag(3, "goat", Some(&animals));
        let sheep = tag(5, "sheep", Some(&animals));
        let other = tag(4, "other", None);
        let all = vec![subject.clone(), animals, goat.clone(), sheep, other];

        assert_eq!(sub_tag_count(&all, subject.id), 3);
        assert_eq!(sub_tag_count(&all, goat.id), 0);
    }

    #[test]
    fn a_tag_knows_its_ancestors_its_branch_and_its_descendants() {
        let subject = tag(1, "subject", None);
        let animals = tag(2, "animals", Some(&subject));
        let goat = tag(3, "goat", Some(&animals));
        let other = tag(4, "other", None);
        let all = vec![
            subject.clone(),
            animals.clone(),
            goat.clone(),
            other.clone(),
        ];

        let names = |found: Vec<&Tag>| found.iter().map(|tag| tag.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(ancestors(&all, &goat)), ["subject", "animals"]);
        assert_eq!(names(branch(&all, goat.id)), ["subject", "animals", "goat"]);
        assert_eq!(
            descendants(&all, subject.id),
            [subject.id, animals.id, goat.id]
        );
        assert!(is_within(&all, goat.id, subject.id));
        assert!(is_within(&all, subject.id, subject.id));
        assert!(!is_within(&all, other.id, subject.id));
    }

    #[test]
    fn the_path_text_names_every_level_from_the_top() {
        let subject = tag(1, "subject", None);
        let animals = tag(2, "animals", Some(&subject));
        let goat = tag(3, "goat", Some(&animals));
        let all = vec![subject.clone(), animals.clone(), goat.clone()];

        assert_eq!(path_text(&all, goat.id), "subject › animals › goat");
        assert_eq!(path_text(&all, subject.id), "subject");
        assert_eq!(path_text(&all, tag(9, "unknown", None).id), "");

        let only_the_ancestry = vec![subject, animals, goat.clone()];
        assert_eq!(
            path_text(&only_the_ancestry, goat.id),
            "subject › animals › goat"
        );
    }
}
