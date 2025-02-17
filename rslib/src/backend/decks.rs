// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use super::Backend;
use crate::{
    backend_proto::{self as pb},
    decks::{Deck, DeckID, DeckSchema11},
    prelude::*,
    scheduler::filtered::FilteredDeckForUpdate,
};
pub(super) use pb::decks_service::Service as DecksService;

impl DecksService for Backend {
    fn add_deck_legacy(&self, input: pb::Json) -> Result<pb::OpChangesWithId> {
        let schema11: DeckSchema11 = serde_json::from_slice(&input.json)?;
        let mut deck: Deck = schema11.into();
        self.with_col(|col| {
            let output = col.add_deck(&mut deck)?;
            Ok(output.map(|_| deck.id.0).into())
        })
    }

    fn add_or_update_deck_legacy(&self, input: pb::AddOrUpdateDeckLegacyIn) -> Result<pb::DeckId> {
        self.with_col(|col| {
            let schema11: DeckSchema11 = serde_json::from_slice(&input.deck)?;
            let mut deck: Deck = schema11.into();
            if input.preserve_usn_and_mtime {
                col.transact_no_undo(|col| {
                    let usn = col.usn()?;
                    col.add_or_update_single_deck_with_existing_id(&mut deck, usn)
                })?;
            } else {
                col.add_or_update_deck(&mut deck)?;
            }
            Ok(pb::DeckId { did: deck.id.0 })
        })
    }

    fn deck_tree(&self, input: pb::DeckTreeIn) -> Result<pb::DeckTreeNode> {
        let lim = if input.top_deck_id > 0 {
            Some(DeckID(input.top_deck_id))
        } else {
            None
        };
        self.with_col(|col| {
            let now = if input.now == 0 {
                None
            } else {
                Some(TimestampSecs(input.now))
            };
            col.deck_tree(now, lim)
        })
    }

    fn deck_tree_legacy(&self, _input: pb::Empty) -> Result<pb::Json> {
        self.with_col(|col| {
            let tree = col.legacy_deck_tree()?;
            serde_json::to_vec(&tree)
                .map_err(Into::into)
                .map(Into::into)
        })
    }

    fn get_all_decks_legacy(&self, _input: pb::Empty) -> Result<pb::Json> {
        self.with_col(|col| {
            let decks = col.storage.get_all_decks_as_schema11()?;
            serde_json::to_vec(&decks).map_err(Into::into)
        })
        .map(Into::into)
    }

    fn get_deck_id_by_name(&self, input: pb::String) -> Result<pb::DeckId> {
        self.with_col(|col| {
            col.get_deck_id(&input.val).and_then(|d| {
                d.ok_or(AnkiError::NotFound)
                    .map(|d| pb::DeckId { did: d.0 })
            })
        })
    }

    fn get_deck_legacy(&self, input: pb::DeckId) -> Result<pb::Json> {
        self.with_col(|col| {
            let deck: DeckSchema11 = col
                .storage
                .get_deck(input.into())?
                .ok_or(AnkiError::NotFound)?
                .into();
            serde_json::to_vec(&deck)
                .map_err(Into::into)
                .map(Into::into)
        })
    }

    fn get_deck_names(&self, input: pb::GetDeckNamesIn) -> Result<pb::DeckNames> {
        self.with_col(|col| {
            let names = if input.include_filtered {
                col.get_all_deck_names(input.skip_empty_default)?
            } else {
                col.get_all_normal_deck_names()?
            };
            Ok(pb::DeckNames {
                entries: names
                    .into_iter()
                    .map(|(id, name)| pb::DeckNameId { id: id.0, name })
                    .collect(),
            })
        })
    }

    fn new_deck_legacy(&self, input: pb::Bool) -> Result<pb::Json> {
        let deck = if input.val {
            Deck::new_filtered()
        } else {
            Deck::new_normal()
        };
        let schema11: DeckSchema11 = deck.into();
        serde_json::to_vec(&schema11)
            .map_err(Into::into)
            .map(Into::into)
    }

    fn remove_decks(&self, input: pb::DeckIDs) -> Result<pb::OpChangesWithCount> {
        self.with_col(|col| col.remove_decks_and_child_decks(&Into::<Vec<DeckID>>::into(input)))
            .map(Into::into)
    }

    fn reparent_decks(&self, input: pb::ReparentDecksIn) -> Result<pb::OpChangesWithCount> {
        let deck_ids: Vec<_> = input.deck_ids.into_iter().map(Into::into).collect();
        let new_parent = if input.new_parent == 0 {
            None
        } else {
            Some(input.new_parent.into())
        };
        self.with_col(|col| col.reparent_decks(&deck_ids, new_parent))
            .map(Into::into)
    }

    fn rename_deck(&self, input: pb::RenameDeckIn) -> Result<pb::OpChanges> {
        self.with_col(|col| col.rename_deck(input.deck_id.into(), &input.new_name))
            .map(Into::into)
    }

    fn get_or_create_filtered_deck(&self, input: pb::DeckId) -> Result<pb::FilteredDeckForUpdate> {
        self.with_col(|col| col.get_or_create_filtered_deck(input.into()))
            .map(Into::into)
    }

    fn add_or_update_filtered_deck(
        &self,
        input: pb::FilteredDeckForUpdate,
    ) -> Result<pb::OpChangesWithId> {
        self.with_col(|col| col.add_or_update_filtered_deck(input.into()))
            .map(|out| out.map(i64::from))
            .map(Into::into)
    }
}

impl From<pb::DeckId> for DeckID {
    fn from(did: pb::DeckId) -> Self {
        DeckID(did.did)
    }
}

impl From<pb::DeckIDs> for Vec<DeckID> {
    fn from(dids: pb::DeckIDs) -> Self {
        dids.dids.into_iter().map(DeckID).collect()
    }
}

impl From<DeckID> for pb::DeckId {
    fn from(did: DeckID) -> Self {
        pb::DeckId { did: did.0 }
    }
}

impl From<FilteredDeckForUpdate> for pb::FilteredDeckForUpdate {
    fn from(deck: FilteredDeckForUpdate) -> Self {
        pb::FilteredDeckForUpdate {
            id: deck.id.into(),
            name: deck.human_name,
            config: Some(deck.config),
        }
    }
}

impl From<pb::FilteredDeckForUpdate> for FilteredDeckForUpdate {
    fn from(deck: pb::FilteredDeckForUpdate) -> Self {
        FilteredDeckForUpdate {
            id: deck.id.into(),
            human_name: deck.name,
            config: deck.config.unwrap_or_default(),
        }
    }
}

// before we can switch to returning protobuf, we need to make sure we're converting the
// deck separators

// fn new_deck(&self, input: pb::Bool) -> Result<pb::Deck> {
//     let deck = if input.val {
//         Deck::new_filtered()
//     } else {
//         Deck::new_normal()
//     };
//     Ok(deck.into())
// }

// impl From<pb::Deck> for Deck {
//     fn from(deck: pb::Deck) -> Self {
//         Self {
//             id: deck.id.into(),
//             name: deck.name,
//             mtime_secs: deck.mtime_secs.into(),
//             usn: deck.usn.into(),
//             common: deck.common.unwrap_or_default(),
//             kind: deck
//                 .kind
//                 .map(Into::into)
//                 .unwrap_or_else(|| DeckKind::Normal(NormalDeck::default())),
//         }
//     }
// }

// impl From<pb::deck::Kind> for DeckKind {
//     fn from(kind: pb::deck::Kind) -> Self {
//         match kind {
//             pb::deck::Kind::Normal(normal) => DeckKind::Normal(normal),
//             pb::deck::Kind::Filtered(filtered) => DeckKind::Filtered(filtered),
//         }
//     }
// }
