// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

mod counts;
mod filtered;
mod schema11;
mod tree;
pub(crate) mod undo;

pub use crate::backend_proto::{
    deck_kind::Kind as DeckKind,
    filtered_deck::{search_term::Order as FilteredSearchOrder, SearchTerm as FilteredSearchTerm},
    Deck as DeckProto, DeckCommon, DeckKind as DeckKindProto, FilteredDeck, NormalDeck,
};
use crate::{backend_proto as pb, markdown::render_markdown, text::sanitize_html_no_images};
use crate::{
    collection::Collection,
    deckconf::DeckConfID,
    define_newtype,
    err::{AnkiError, Result},
    i18n::TR,
    prelude::*,
    text::normalize_to_nfc,
    timestamp::TimestampSecs,
    types::Usn,
};
pub(crate) use counts::DueCounts;
pub use schema11::DeckSchema11;
use std::{borrow::Cow, sync::Arc};

define_newtype!(DeckID, i64);

#[derive(Debug, Clone, PartialEq)]
pub struct Deck {
    pub id: DeckID,
    pub name: String,
    pub mtime_secs: TimestampSecs,
    pub usn: Usn,
    pub common: DeckCommon,
    pub kind: DeckKind,
}

impl Deck {
    pub fn new_normal() -> Deck {
        Deck {
            id: DeckID(0),
            name: "".into(),
            mtime_secs: TimestampSecs(0),
            usn: Usn(0),
            common: DeckCommon {
                study_collapsed: true,
                browser_collapsed: true,
                ..Default::default()
            },
            kind: DeckKind::Normal(NormalDeck {
                config_id: 1,
                // enable in the future
                // markdown_description = true,
                ..Default::default()
            }),
        }
    }

    fn reset_stats_if_day_changed(&mut self, today: u32) {
        let c = &mut self.common;
        if c.last_day_studied != today {
            c.new_studied = 0;
            c.learning_studied = 0;
            c.review_studied = 0;
            c.milliseconds_studied = 0;
            c.last_day_studied = today;
        }
    }

    /// Returns deck config ID if deck is a normal deck.
    pub(crate) fn config_id(&self) -> Option<DeckConfID> {
        if let DeckKind::Normal(ref norm) = self.kind {
            Some(DeckConfID(norm.config_id))
        } else {
            None
        }
    }

    // used by tests at the moment

    #[allow(dead_code)]
    pub(crate) fn normal(&self) -> Result<&NormalDeck> {
        if let DeckKind::Normal(normal) = &self.kind {
            Ok(normal)
        } else {
            Err(AnkiError::invalid_input("deck not normal"))
        }
    }

    #[allow(dead_code)]
    pub(crate) fn normal_mut(&mut self) -> Result<&mut NormalDeck> {
        if let DeckKind::Normal(normal) = &mut self.kind {
            Ok(normal)
        } else {
            Err(AnkiError::invalid_input("deck not normal"))
        }
    }

    pub(crate) fn filtered(&self) -> Result<&FilteredDeck> {
        if let DeckKind::Filtered(filtered) = &self.kind {
            Ok(filtered)
        } else {
            Err(AnkiError::invalid_input("deck not filtered"))
        }
    }

    #[allow(dead_code)]
    pub(crate) fn filtered_mut(&mut self) -> Result<&mut FilteredDeck> {
        if let DeckKind::Filtered(filtered) = &mut self.kind {
            Ok(filtered)
        } else {
            Err(AnkiError::invalid_input("deck not filtered"))
        }
    }

    pub fn human_name(&self) -> String {
        self.name.replace("\x1f", "::")
    }

    pub(crate) fn set_modified(&mut self, usn: Usn) {
        self.mtime_secs = TimestampSecs::now();
        self.usn = usn;
    }

    /// Return the studied counts if studied today.
    /// May be negative if user has extended limits.
    pub(crate) fn new_rev_counts(&self, today: u32) -> (i32, i32) {
        if self.common.last_day_studied == today {
            (self.common.new_studied, self.common.review_studied)
        } else {
            (0, 0)
        }
    }

    pub fn rendered_description(&self) -> String {
        if let DeckKind::Normal(normal) = &self.kind {
            if normal.markdown_description {
                let description = render_markdown(&normal.description);
                // before allowing images, we'll need to handle relative image
                // links on the various platforms
                sanitize_html_no_images(&description)
            } else {
                String::new()
            }
        } else {
            String::new()
        }
    }
}

fn invalid_char_for_deck_component(c: char) -> bool {
    c.is_ascii_control() || c == '"'
}

fn normalized_deck_name_component(comp: &str) -> Cow<str> {
    let mut out = normalize_to_nfc(comp);
    if out.contains(invalid_char_for_deck_component) {
        out = out.replace(invalid_char_for_deck_component, "").into();
    }
    let trimmed = out.trim();
    if trimmed.is_empty() {
        "blank".to_string().into()
    } else if trimmed.len() != out.len() {
        trimmed.to_string().into()
    } else {
        out
    }
}

fn normalize_native_name(name: &str) -> Cow<str> {
    if name
        .split('\x1f')
        .any(|comp| matches!(normalized_deck_name_component(comp), Cow::Owned(_)))
    {
        let comps: Vec<_> = name
            .split('\x1f')
            .map(normalized_deck_name_component)
            .collect::<Vec<_>>();
        comps.join("\x1f").into()
    } else {
        // no changes required
        name.into()
    }
}

pub(crate) fn human_deck_name_to_native(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for comp in name.split("::") {
        out.push_str(&normalized_deck_name_component(comp));
        out.push('\x1f');
    }
    out.trim_end_matches('\x1f').into()
}

impl Collection {
    pub(crate) fn get_deck(&mut self, did: DeckID) -> Result<Option<Arc<Deck>>> {
        if let Some(deck) = self.state.deck_cache.get(&did) {
            return Ok(Some(deck.clone()));
        }
        if let Some(deck) = self.storage.get_deck(did)? {
            let deck = Arc::new(deck);
            self.state.deck_cache.insert(did, deck.clone());
            Ok(Some(deck))
        } else {
            Ok(None)
        }
    }
}

impl From<Deck> for DeckProto {
    fn from(d: Deck) -> Self {
        DeckProto {
            id: d.id.0,
            name: d.name,
            mtime_secs: d.mtime_secs.0,
            usn: d.usn.0,
            common: Some(d.common),
            kind: Some(d.kind.into()),
        }
    }
}

impl From<DeckKind> for pb::deck::Kind {
    fn from(k: DeckKind) -> Self {
        match k {
            DeckKind::Normal(n) => pb::deck::Kind::Normal(n),
            DeckKind::Filtered(f) => pb::deck::Kind::Filtered(f),
        }
    }
}

pub(crate) fn immediate_parent_name(machine_name: &str) -> Option<&str> {
    machine_name.rsplitn(2, '\x1f').nth(1)
}

/// Determine name to rename a deck to, when `dragged` is dropped on `dropped`.
/// `dropped` being unset represents a drop at the top or bottom of the deck list.
/// The returned name should be used to rename `dragged`.
/// Arguments are expected in 'machine' form with an \x1f separator.
pub(crate) fn reparented_name(dragged: &str, dropped: Option<&str>) -> Option<String> {
    let dragged_base = dragged.rsplit('\x1f').next().unwrap();
    if let Some(dropped) = dropped {
        if dropped.starts_with(dragged) {
            // foo onto foo::bar, or foo onto itself -> no-op
            None
        } else {
            // foo::bar onto baz -> baz::bar
            Some(format!("{}\x1f{}", dropped, dragged_base))
        }
    } else {
        // foo::bar onto top level -> bar
        Some(dragged_base.into())
    }
}

impl Collection {
    pub(crate) fn default_deck_is_empty(&self) -> Result<bool> {
        self.storage.deck_is_empty(DeckID(1))
    }

    /// Normalize deck name and rename if not unique. Bumps mtime and usn if
    /// name was changed, but otherwise leaves it the same.
    fn prepare_deck_for_update(&mut self, deck: &mut Deck, usn: Usn) -> Result<()> {
        if let Cow::Owned(name) = normalize_native_name(&deck.name) {
            deck.name = name;
            deck.set_modified(usn);
        }
        self.ensure_deck_name_unique(deck, usn)
    }

    /// Add or update an existing deck modified by the user. May add parents,
    /// or rename children as required. Prefer add_deck() or update_deck() to
    /// be explicit about your intentions; this function mainly exists so we
    /// can integrate with older Python code that behaved this way.
    pub(crate) fn add_or_update_deck(&mut self, deck: &mut Deck) -> Result<OpOutput<()>> {
        if deck.id.0 == 0 {
            self.add_deck(deck)
        } else {
            self.update_deck(deck)
        }
    }

    /// Add a new deck. The id must be 0, as it will be automatically assigned.
    pub fn add_deck(&mut self, deck: &mut Deck) -> Result<OpOutput<()>> {
        if deck.id.0 != 0 {
            return Err(AnkiError::invalid_input("deck to add must have id 0"));
        }

        self.transact(Op::AddDeck, |col| col.add_deck_inner(deck, col.usn()?))
    }

    pub(crate) fn add_deck_inner(&mut self, deck: &mut Deck, usn: Usn) -> Result<()> {
        self.prepare_deck_for_update(deck, usn)?;
        deck.set_modified(usn);
        self.match_or_create_parents(deck, usn)?;
        self.add_deck_undoable(deck)
    }

    pub fn update_deck(&mut self, deck: &mut Deck) -> Result<OpOutput<()>> {
        self.transact(Op::UpdateDeck, |col| {
            let existing_deck = col.storage.get_deck(deck.id)?.ok_or(AnkiError::NotFound)?;
            col.update_deck_inner(deck, existing_deck, col.usn()?)
        })
    }

    pub fn rename_deck(&mut self, did: DeckID, new_human_name: &str) -> Result<OpOutput<()>> {
        self.transact(Op::RenameDeck, |col| {
            let existing_deck = col.storage.get_deck(did)?.ok_or(AnkiError::NotFound)?;
            let mut deck = existing_deck.clone();
            deck.name = human_deck_name_to_native(new_human_name);
            col.update_deck_inner(&mut deck, existing_deck, col.usn()?)
        })
    }

    pub(crate) fn update_deck_inner(
        &mut self,
        deck: &mut Deck,
        original: Deck,
        usn: Usn,
    ) -> Result<()> {
        self.prepare_deck_for_update(deck, usn)?;
        deck.set_modified(usn);
        let name_changed = original.name != deck.name;
        if name_changed {
            // match closest parent name
            self.match_or_create_parents(deck, usn)?;
            // rename children
            self.rename_child_decks(&original, &deck.name, usn)?;
        }
        self.update_single_deck_undoable(deck, original)?;
        if name_changed {
            // after updating, we need to ensure all grandparents exist, which may not be the case
            // in the parent->child case
            self.create_missing_parents(&deck.name, usn)?;
        }
        Ok(())
    }

    /// Add/update a single deck when syncing/importing. Ensures name is unique
    /// & normalized, but does not check parents/children or update mtime
    /// (unless the name was changed). Caller must set up transaction.
    pub(crate) fn add_or_update_single_deck_with_existing_id(
        &mut self,
        deck: &mut Deck,
        usn: Usn,
    ) -> Result<()> {
        self.prepare_deck_for_update(deck, usn)?;
        self.add_or_update_deck_with_existing_id_undoable(deck)
    }

    pub(crate) fn ensure_deck_name_unique(&self, deck: &mut Deck, usn: Usn) -> Result<()> {
        loop {
            match self.storage.get_deck_id(&deck.name)? {
                Some(did) if did == deck.id => {
                    break;
                }
                None => break,
                _ => (),
            }
            deck.name += "+";
            deck.set_modified(usn);
        }

        Ok(())
    }

    pub(crate) fn recover_missing_deck(&mut self, did: DeckID, usn: Usn) -> Result<()> {
        let mut deck = Deck::new_normal();
        deck.id = did;
        deck.name = format!("recovered{}", did);
        deck.set_modified(usn);
        self.add_or_update_single_deck_with_existing_id(&mut deck, usn)
    }

    pub fn get_or_create_normal_deck(&mut self, human_name: &str) -> Result<Deck> {
        let native_name = human_deck_name_to_native(human_name);
        if let Some(did) = self.storage.get_deck_id(&native_name)? {
            self.storage.get_deck(did).map(|opt| opt.unwrap())
        } else {
            let mut deck = Deck::new_normal();
            deck.name = native_name;
            self.add_or_update_deck(&mut deck)?;
            Ok(deck)
        }
    }

    fn rename_child_decks(&mut self, old: &Deck, new_name: &str, usn: Usn) -> Result<()> {
        let children = self.storage.child_decks(old)?;
        let old_component_count = old.name.matches('\x1f').count() + 1;

        for mut child in children {
            let original = child.clone();
            let child_components: Vec<_> = child.name.split('\x1f').collect();
            let child_only = &child_components[old_component_count..];
            let new_name = format!("{}\x1f{}", new_name, child_only.join("\x1f"));
            child.name = new_name;
            child.set_modified(usn);
            self.update_single_deck_undoable(&mut child, original)?;
        }

        Ok(())
    }

    /// Add a single, normal deck with the provided name for a child deck.
    /// Caller must have done necessarily validation on name.
    fn add_parent_deck(&mut self, machine_name: &str, usn: Usn) -> Result<()> {
        let mut deck = Deck::new_normal();
        deck.name = machine_name.into();
        deck.set_modified(usn);
        self.add_deck_undoable(&mut deck)
    }

    /// If parent deck(s) exist, rewrite name to match their case.
    /// If they don't exist, create them.
    /// Returns an error if a DB operation fails, or if the first existing parent is a filtered deck.
    fn match_or_create_parents(&mut self, deck: &mut Deck, usn: Usn) -> Result<()> {
        let child_split: Vec<_> = deck.name.split('\x1f').collect();
        if let Some(parent_deck) = self.first_existing_parent(&deck.name, 0)? {
            if parent_deck.is_filtered() {
                return Err(AnkiError::DeckIsFiltered);
            }
            let parent_count = parent_deck.name.matches('\x1f').count() + 1;
            let need_create = parent_count != child_split.len() - 1;
            deck.name = format!(
                "{}\x1f{}",
                parent_deck.name,
                &child_split[parent_count..].join("\x1f")
            );
            if need_create {
                self.create_missing_parents(&deck.name, usn)?;
            }
            Ok(())
        } else if child_split.len() == 1 {
            // no parents required
            Ok(())
        } else {
            // no existing parents
            self.create_missing_parents(&deck.name, usn)
        }
    }

    fn create_missing_parents(&mut self, mut machine_name: &str, usn: Usn) -> Result<()> {
        while let Some(parent_name) = immediate_parent_name(machine_name) {
            if self.storage.get_deck_id(parent_name)?.is_none() {
                self.add_parent_deck(parent_name, usn)?;
            }
            machine_name = parent_name;
        }
        Ok(())
    }

    fn first_existing_parent(
        &self,
        machine_name: &str,
        recursion_level: usize,
    ) -> Result<Option<Deck>> {
        if recursion_level > 10 {
            return Err(AnkiError::invalid_input("deck nesting level too deep"));
        }
        if let Some(parent_name) = immediate_parent_name(machine_name) {
            if let Some(parent_did) = self.storage.get_deck_id(parent_name)? {
                self.storage.get_deck(parent_did)
            } else {
                self.first_existing_parent(parent_name, recursion_level + 1)
            }
        } else {
            Ok(None)
        }
    }

    /// Get a deck based on its human name. If you have a machine name,
    /// use the method in storage instead.
    pub(crate) fn get_deck_id(&self, human_name: &str) -> Result<Option<DeckID>> {
        let machine_name = human_deck_name_to_native(&human_name);
        self.storage.get_deck_id(&machine_name)
    }

    pub fn remove_decks_and_child_decks(&mut self, dids: &[DeckID]) -> Result<OpOutput<usize>> {
        self.transact(Op::RemoveDeck, |col| {
            let mut card_count = 0;
            let usn = col.usn()?;
            for did in dids {
                if let Some(deck) = col.storage.get_deck(*did)? {
                    let child_decks = col.storage.child_decks(&deck)?;

                    // top level
                    card_count += col.remove_single_deck(&deck, usn)?;

                    // remove children
                    for deck in child_decks {
                        card_count += col.remove_single_deck(&deck, usn)?;
                    }
                }
            }
            Ok(card_count)
        })
    }

    pub(crate) fn remove_single_deck(&mut self, deck: &Deck, usn: Usn) -> Result<usize> {
        let card_count = match deck.kind {
            DeckKind::Normal(_) => self.delete_all_cards_in_normal_deck(deck.id)?,
            DeckKind::Filtered(_) => {
                self.return_all_cards_in_filtered_deck(deck.id)?;
                0
            }
        };
        self.clear_aux_config_for_deck(deck.id)?;
        if deck.id.0 == 1 {
            // if deleting the default deck, ensure there's a new one, and avoid the grave
            let mut deck = deck.to_owned();
            deck.name = self.i18n.tr(TR::DeckConfigDefaultName).into();
            deck.set_modified(usn);
            self.add_or_update_single_deck_with_existing_id(&mut deck, usn)?;
        } else {
            self.remove_deck_and_add_grave_undoable(deck.clone(), usn)?;
        }
        Ok(card_count)
    }

    fn delete_all_cards_in_normal_deck(&mut self, did: DeckID) -> Result<usize> {
        let cids = self.storage.all_cards_in_single_deck(did)?;
        self.remove_cards_and_orphaned_notes(&cids)?;
        Ok(cids.len())
    }

    pub fn get_all_deck_names(&self, skip_empty_default: bool) -> Result<Vec<(DeckID, String)>> {
        if skip_empty_default && self.default_deck_is_empty()? {
            Ok(self
                .storage
                .get_all_deck_names()?
                .into_iter()
                .filter(|(id, _name)| id.0 != 1)
                .collect())
        } else {
            self.storage.get_all_deck_names()
        }
    }

    pub fn get_all_normal_deck_names(&mut self) -> Result<Vec<(DeckID, String)>> {
        Ok(self
            .storage
            .get_all_deck_names()?
            .into_iter()
            .filter(|(id, _name)| match self.get_deck(*id) {
                Ok(Some(deck)) => !deck.is_filtered(),
                _ => true,
            })
            .collect())
    }

    /// Apply input delta to deck, and its parents.
    /// Caller should ensure transaction.
    pub(crate) fn update_deck_stats(
        &mut self,
        today: u32,
        usn: Usn,
        input: pb::UpdateStatsIn,
    ) -> Result<()> {
        let did = input.deck_id.into();
        let mutator = |c: &mut DeckCommon| {
            c.new_studied += input.new_delta;
            c.review_studied += input.review_delta;
            c.milliseconds_studied += input.millisecond_delta;
        };
        if let Some(mut deck) = self.storage.get_deck(did)? {
            self.update_deck_stats_single(today, usn, &mut deck, mutator)?;
            for mut deck in self.storage.parent_decks(&deck)? {
                self.update_deck_stats_single(today, usn, &mut deck, mutator)?;
            }
        }
        Ok(())
    }

    /// Modify the deck's limits by adjusting the 'done today' count.
    /// Positive values increase the limit, negative value decrease it.
    /// Caller should ensure a transaction.
    pub(crate) fn extend_limits(
        &mut self,
        today: u32,
        usn: Usn,
        did: DeckID,
        new_delta: i32,
        review_delta: i32,
    ) -> Result<()> {
        let mutator = |c: &mut DeckCommon| {
            c.new_studied -= new_delta;
            c.review_studied -= review_delta;
        };
        if let Some(mut deck) = self.storage.get_deck(did)? {
            self.update_deck_stats_single(today, usn, &mut deck, mutator)?;
            for mut deck in self.storage.parent_decks(&deck)? {
                self.update_deck_stats_single(today, usn, &mut deck, mutator)?;
            }
            for mut deck in self.storage.child_decks(&deck)? {
                self.update_deck_stats_single(today, usn, &mut deck, mutator)?;
            }
        }

        Ok(())
    }

    pub(crate) fn counts_for_deck_today(
        &mut self,
        did: DeckID,
    ) -> Result<pb::CountsForDeckTodayOut> {
        let today = self.current_due_day(0)?;
        let mut deck = self.storage.get_deck(did)?.ok_or(AnkiError::NotFound)?;
        deck.reset_stats_if_day_changed(today);
        Ok(pb::CountsForDeckTodayOut {
            new: deck.common.new_studied,
            review: deck.common.review_studied,
        })
    }

    fn update_deck_stats_single<F>(
        &mut self,
        today: u32,
        usn: Usn,
        deck: &mut Deck,
        mutator: F,
    ) -> Result<()>
    where
        F: FnOnce(&mut DeckCommon),
    {
        let original = deck.clone();
        deck.reset_stats_if_day_changed(today);
        mutator(&mut deck.common);
        deck.set_modified(usn);
        self.update_single_deck_undoable(deck, original)
    }

    pub fn reparent_decks(
        &mut self,
        deck_ids: &[DeckID],
        new_parent: Option<DeckID>,
    ) -> Result<OpOutput<usize>> {
        self.transact(Op::ReparentDeck, |col| {
            col.reparent_decks_inner(deck_ids, new_parent)
        })
    }

    pub fn reparent_decks_inner(
        &mut self,
        deck_ids: &[DeckID],
        new_parent: Option<DeckID>,
    ) -> Result<usize> {
        let usn = self.usn()?;
        let target_deck;
        let mut target_name = None;
        if let Some(target) = new_parent {
            if let Some(target) = self.storage.get_deck(target)? {
                if target.is_filtered() {
                    return Err(AnkiError::DeckIsFiltered);
                }
                target_deck = target;
                target_name = Some(target_deck.name.as_str());
            }
        }

        let mut count = 0;
        for deck in deck_ids {
            if let Some(mut deck) = self.storage.get_deck(*deck)? {
                if let Some(new_name) = reparented_name(&deck.name, target_name) {
                    count += 1;
                    let orig = deck.clone();

                    // this is basically update_deck_inner(), except:
                    // - we skip the normalization in prepare_for_update()
                    // - we skip the match_or_create_parents() step
                    // - we skip the final create_missing_parents(), as we don't allow parent->child
                    //   renames

                    deck.set_modified(usn);
                    deck.name = new_name;
                    self.ensure_deck_name_unique(&mut deck, usn)?;
                    self.rename_child_decks(&orig, &deck.name, usn)?;
                    self.update_single_deck_undoable(&mut deck, orig)?;
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod test {
    use super::{human_deck_name_to_native, immediate_parent_name, normalize_native_name};
    use crate::decks::reparented_name;
    use crate::{
        collection::{open_test_collection, Collection},
        err::Result,
        search::SortMode,
    };

    fn sorted_names(col: &Collection) -> Vec<String> {
        col.storage
            .get_all_deck_names()
            .unwrap()
            .into_iter()
            .map(|d| d.1)
            .collect()
    }

    #[test]
    fn parent() {
        assert_eq!(immediate_parent_name("foo"), None);
        assert_eq!(immediate_parent_name("foo\x1fbar"), Some("foo"));
        assert_eq!(
            immediate_parent_name("foo\x1fbar\x1fbaz"),
            Some("foo\x1fbar")
        );
    }

    #[test]
    fn from_human() {
        assert_eq!(&human_deck_name_to_native("foo"), "foo");
        assert_eq!(&human_deck_name_to_native("foo::bar"), "foo\x1fbar");
        assert_eq!(&human_deck_name_to_native("fo\x1fo::ba\nr"), "foo\x1fbar");
        assert_eq!(
            &human_deck_name_to_native("foo::::baz"),
            "foo\x1fblank\x1fbaz"
        );
    }

    #[test]
    fn normalize() {
        assert_eq!(&normalize_native_name("foo\x1fbar"), "foo\x1fbar");
        assert_eq!(&normalize_native_name("fo\u{a}o\x1fbar"), "foo\x1fbar");
    }

    #[test]
    fn adding_updating() -> Result<()> {
        let mut col = open_test_collection();

        let deck1 = col.get_or_create_normal_deck("foo")?;
        let deck2 = col.get_or_create_normal_deck("FOO")?;
        assert_eq!(deck1.id, deck2.id);
        assert_eq!(sorted_names(&col), vec!["Default", "foo"]);

        // missing parents should be automatically created, and case should match
        // existing parents
        let _deck3 = col.get_or_create_normal_deck("FOO::BAR::BAZ")?;
        assert_eq!(
            sorted_names(&col),
            vec!["Default", "foo", "foo::BAR", "foo::BAR::BAZ"]
        );

        Ok(())
    }

    #[test]
    fn renaming() -> Result<()> {
        let mut col = open_test_collection();

        let _ = col.get_or_create_normal_deck("foo::bar::baz")?;
        let mut top_deck = col.get_or_create_normal_deck("foo")?;
        top_deck.name = "other".into();
        col.add_or_update_deck(&mut top_deck)?;
        assert_eq!(
            sorted_names(&col),
            vec!["Default", "other", "other::bar", "other::bar::baz"]
        );

        // should do the right thing in the middle of the tree as well
        let mut middle = col.get_or_create_normal_deck("other::bar")?;
        middle.name = "quux\x1ffoo".into();
        col.add_or_update_deck(&mut middle)?;
        assert_eq!(
            sorted_names(&col),
            vec!["Default", "other", "quux", "quux::foo", "quux::foo::baz"]
        );

        // add another child
        let _ = col.get_or_create_normal_deck("quux::foo::baz2");

        // quux::foo -> quux::foo::baz::four
        // means quux::foo::baz2 should be quux::foo::baz::four::baz2
        // and a new quux::foo should have been created
        middle.name = "quux\x1ffoo\x1fbaz\x1ffour".into();
        col.add_or_update_deck(&mut middle)?;
        assert_eq!(
            sorted_names(&col),
            vec![
                "Default",
                "other",
                "quux",
                "quux::foo",
                "quux::foo::baz",
                "quux::foo::baz::four",
                "quux::foo::baz::four::baz",
                "quux::foo::baz::four::baz2"
            ]
        );

        // should handle name conflicts
        middle.name = "other".into();
        col.add_or_update_deck(&mut middle)?;
        assert_eq!(middle.name, "other+");

        // public function takes human name
        col.rename_deck(middle.id, "one::two")?;
        assert_eq!(
            sorted_names(&col),
            vec![
                "Default",
                "one",
                "one::two",
                "one::two::baz",
                "one::two::baz2",
                "other",
                "quux",
                "quux::foo",
                "quux::foo::baz",
            ]
        );

        Ok(())
    }

    #[test]
    fn default() -> Result<()> {
        // deleting the default deck will remove cards, but bring the deck back
        // as a top level deck
        let mut col = open_test_collection();

        let mut default = col.get_or_create_normal_deck("default")?;
        default.name = "one\x1ftwo".into();
        col.add_or_update_deck(&mut default)?;

        // create a non-default deck confusingly named "default"
        let _fake_default = col.get_or_create_normal_deck("default")?;

        // add a card to the real default
        let nt = col.get_notetype_by_name("Basic")?.unwrap();
        let mut note = nt.new_note();
        col.add_note(&mut note, default.id)?;
        assert_ne!(col.search_cards("", SortMode::NoOrder)?, vec![]);

        // add a subdeck
        let _ = col.get_or_create_normal_deck("one::two::three")?;

        // delete top level
        let top = col.get_or_create_normal_deck("one")?;
        col.remove_decks_and_child_decks(&[top.id])?;

        // should have come back as "Default+" due to conflict
        assert_eq!(sorted_names(&col), vec!["default", "Default+"]);

        // and the cards it contained should have been removed
        assert_eq!(col.search_cards("", SortMode::NoOrder)?, vec![]);

        Ok(())
    }

    #[test]
    fn drag_drop() {
        // use custom separator to make the tests easier to read
        fn n(s: &str) -> String {
            s.replace(":", "\x1f")
        }
        fn n_opt(s: &str) -> Option<String> {
            Some(n(s))
        }

        assert_eq!(reparented_name("drag", Some("drop")), n_opt("drop:drag"));
        assert_eq!(reparented_name("drag", None), n_opt("drag"));
        assert_eq!(reparented_name(&n("drag:child"), None), n_opt("child"));
        assert_eq!(
            reparented_name(&n("drag:child"), Some(&n("drop:deck"))),
            n_opt("drop:deck:child")
        );
        assert_eq!(
            reparented_name(&n("drag:child"), Some("drag")),
            n_opt("drag:child")
        );
        assert_eq!(
            reparented_name(&n("drag:child:grandchild"), Some("drag")),
            n_opt("drag:grandchild")
        );
        // drops to child not supported
        assert_eq!(
            reparented_name(&n("drag"), Some(&n("drag:child:grandchild"))),
            None
        );
        // name doesn't change when deck dropped on itself
        assert_eq!(reparented_name(&n("foo:bar"), Some(&n("foo:bar"))), None);
    }
}
