// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use std::{collections::HashSet, iter::Peekable};

use unicase::UniCase;

use super::{immediate_parent_name_unicase, Tag};
use crate::{backend_proto::TagTreeNode, prelude::*};

impl Collection {
    pub fn tag_tree(&mut self) -> Result<TagTreeNode> {
        let tags = self.storage.all_tags()?;
        let tree = tags_to_tree(tags);

        Ok(tree)
    }
}

/// Append any missing parents. Caller must sort afterwards.
fn add_missing_parents(tags: &mut Vec<Tag>) {
    let mut all_names: HashSet<UniCase<&str>> = HashSet::new();
    let mut missing = vec![];
    for tag in &*tags {
        add_tag_and_missing_parents(&mut all_names, &mut missing, UniCase::new(&tag.name))
    }
    let mut missing: Vec<_> = missing
        .into_iter()
        .map(|n| Tag::new(n.to_string(), Usn(0)))
        .collect();
    tags.append(&mut missing);
}

fn tags_to_tree(mut tags: Vec<Tag>) -> TagTreeNode {
    for tag in &mut tags {
        tag.name = tag.name.replace("::", "\x1f");
    }
    add_missing_parents(&mut tags);
    tags.sort_unstable_by(|a, b| UniCase::new(&a.name).cmp(&UniCase::new(&b.name)));
    let mut top = TagTreeNode::default();
    let mut it = tags.into_iter().peekable();
    add_child_nodes(&mut it, &mut top);

    top
}

fn add_child_nodes(tags: &mut Peekable<impl Iterator<Item = Tag>>, parent: &mut TagTreeNode) {
    while let Some(tag) = tags.peek() {
        let split_name: Vec<_> = tag.name.split('\x1f').collect();
        match split_name.len() as u32 {
            l if l <= parent.level => {
                // next item is at a higher level
                return;
            }
            l if l == parent.level + 1 => {
                // next item is an immediate descendent of parent
                parent.children.push(TagTreeNode {
                    name: (*split_name.last().unwrap()).into(),
                    children: vec![],
                    level: parent.level + 1,
                    expanded: tag.expanded,
                });
                tags.next();
            }
            _ => {
                // next item is at a lower level
                if let Some(last_child) = parent.children.last_mut() {
                    add_child_nodes(tags, last_child)
                } else {
                    // immediate parent is missing
                    tags.next();
                }
            }
        }
    }
}

/// For the given tag, check if immediate parent exists. If so, add
/// tag and return.
/// If the immediate parent is missing, check and add any missing parents.
/// This should ensure that if an immediate parent is found, all ancestors
/// are guaranteed to already exist.
fn add_tag_and_missing_parents<'a, 'b>(
    all: &'a mut HashSet<UniCase<&'b str>>,
    missing: &'a mut Vec<UniCase<&'b str>>,
    tag_name: UniCase<&'b str>,
) {
    if let Some(parent) = immediate_parent_name_unicase(tag_name) {
        if !all.contains(&parent) {
            missing.push(parent);
            add_tag_and_missing_parents(all, missing, parent);
        }
    }
    // finally, add provided tag
    all.insert(tag_name);
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::collection::open_test_collection;

    fn node(name: &str, level: u32, children: Vec<TagTreeNode>) -> TagTreeNode {
        TagTreeNode {
            name: name.into(),
            level,
            children,

            ..Default::default()
        }
    }

    fn leaf(name: &str, level: u32) -> TagTreeNode {
        node(name, level, vec![])
    }

    #[test]
    fn tree() -> Result<()> {
        let mut col = open_test_collection();
        let nt = col.get_notetype_by_name("Basic")?.unwrap();
        let mut note = nt.new_note();
        note.tags.push("foo::bar::a".into());
        note.tags.push("foo::bar::b".into());
        col.add_note(&mut note, DeckID(1))?;

        // missing parents are added
        assert_eq!(
            col.tag_tree()?,
            node(
                "",
                0,
                vec![node(
                    "foo",
                    1,
                    vec![node("bar", 2, vec![leaf("a", 3), leaf("b", 3)])]
                )]
            )
        );

        // differing case should result in only one parent case being added -
        // the first one
        col.storage.clear_all_tags()?;
        note.tags[0] = "foo::BAR::a".into();
        note.tags[1] = "FOO::bar::b".into();
        col.update_note(&mut note)?;
        assert_eq!(
            col.tag_tree()?,
            node(
                "",
                0,
                vec![node(
                    "foo",
                    1,
                    vec![node("BAR", 2, vec![leaf("a", 3), leaf("b", 3)])]
                )]
            )
        );

        // things should work even if the immediate parent is not missing
        col.storage.clear_all_tags()?;
        note.tags[0] = "foo::bar::baz".into();
        note.tags[1] = "foo::bar::baz::quux".into();
        col.update_note(&mut note)?;
        assert_eq!(
            col.tag_tree()?,
            node(
                "",
                0,
                vec![node(
                    "foo",
                    1,
                    vec![node("bar", 2, vec![node("baz", 3, vec![leaf("quux", 4)])])]
                )]
            )
        );

        // numbers have a smaller ascii number than ':', so a naive sort on
        // '::' would result in one::two being nested under one1.
        col.storage.clear_all_tags()?;
        note.tags[0] = "one".into();
        note.tags[1] = "one1".into();
        note.tags.push("one::two".into());
        col.update_note(&mut note)?;
        assert_eq!(
            col.tag_tree()?,
            node(
                "",
                0,
                vec![node("one", 1, vec![leaf("two", 2)]), leaf("one1", 1)]
            )
        );

        // children should match the case of their parents
        col.storage.clear_all_tags()?;
        note.tags[0] = "FOO".into();
        note.tags[1] = "foo::BAR".into();
        note.tags[2] = "foo::bar::baz".into();
        col.update_note(&mut note)?;
        assert_eq!(note.tags, vec!["FOO", "FOO::BAR", "FOO::BAR::baz"]);

        Ok(())
    }
}
