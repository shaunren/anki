// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

mod answering;
mod states;

use super::Backend;
use crate::{
    backend_proto::{self as pb},
    prelude::*,
    scheduler::{
        new::NewCardSortOrder,
        states::{CardState, NextCardStates},
    },
    stats::studied_today,
};
pub(super) use pb::scheduling_service::Service as SchedulingService;

impl SchedulingService for Backend {
    /// This behaves like _updateCutoff() in older code - it also unburies at the start of
    /// a new day.
    fn sched_timing_today(&self, _input: pb::Empty) -> Result<pb::SchedTimingTodayOut> {
        self.with_col(|col| {
            let timing = col.timing_today()?;
            col.unbury_if_day_rolled_over(timing)?;
            Ok(timing.into())
        })
    }

    /// Fetch data from DB and return rendered string.
    fn studied_today(&self, _input: pb::Empty) -> Result<pb::String> {
        self.with_col(|col| col.studied_today().map(Into::into))
    }

    /// Message rendering only, for old graphs.
    fn studied_today_message(&self, input: pb::StudiedTodayMessageIn) -> Result<pb::String> {
        Ok(studied_today(input.cards, input.seconds as f32, &self.i18n).into())
    }

    fn update_stats(&self, input: pb::UpdateStatsIn) -> Result<pb::Empty> {
        self.with_col(|col| {
            col.transact_no_undo(|col| {
                let today = col.current_due_day(0)?;
                let usn = col.usn()?;
                col.update_deck_stats(today, usn, input).map(Into::into)
            })
        })
    }

    fn extend_limits(&self, input: pb::ExtendLimitsIn) -> Result<pb::Empty> {
        self.with_col(|col| {
            col.transact_no_undo(|col| {
                let today = col.current_due_day(0)?;
                let usn = col.usn()?;
                col.extend_limits(
                    today,
                    usn,
                    input.deck_id.into(),
                    input.new_delta,
                    input.review_delta,
                )
                .map(Into::into)
            })
        })
    }

    fn counts_for_deck_today(&self, input: pb::DeckId) -> Result<pb::CountsForDeckTodayOut> {
        self.with_col(|col| col.counts_for_deck_today(input.did.into()))
    }

    fn congrats_info(&self, _input: pb::Empty) -> Result<pb::CongratsInfoOut> {
        self.with_col(|col| col.congrats_info())
    }

    fn restore_buried_and_suspended_cards(&self, input: pb::CardIDs) -> Result<pb::OpChanges> {
        let cids: Vec<_> = input.into();
        self.with_col(|col| col.unbury_or_unsuspend_cards(&cids).map(Into::into))
    }

    fn unbury_cards_in_current_deck(
        &self,
        input: pb::UnburyCardsInCurrentDeckIn,
    ) -> Result<pb::Empty> {
        self.with_col(|col| {
            col.unbury_cards_in_current_deck(input.mode())
                .map(Into::into)
        })
    }

    fn bury_or_suspend_cards(&self, input: pb::BuryOrSuspendCardsIn) -> Result<pb::OpChanges> {
        self.with_col(|col| {
            let mode = input.mode();
            let cids: Vec<_> = input.card_ids.into_iter().map(CardID).collect();
            col.bury_or_suspend_cards(&cids, mode).map(Into::into)
        })
    }

    fn empty_filtered_deck(&self, input: pb::DeckId) -> Result<pb::OpChanges> {
        self.with_col(|col| col.empty_filtered_deck(input.did.into()).map(Into::into))
    }

    fn rebuild_filtered_deck(&self, input: pb::DeckId) -> Result<pb::OpChangesWithCount> {
        self.with_col(|col| col.rebuild_filtered_deck(input.did.into()).map(Into::into))
    }

    fn schedule_cards_as_new(&self, input: pb::ScheduleCardsAsNewIn) -> Result<pb::OpChanges> {
        self.with_col(|col| {
            let cids: Vec<_> = input.card_ids.into_iter().map(CardID).collect();
            let log = input.log;
            col.reschedule_cards_as_new(&cids, log).map(Into::into)
        })
    }

    fn set_due_date(&self, input: pb::SetDueDateIn) -> Result<pb::OpChanges> {
        let config = input.config_key.map(Into::into);
        let days = input.days;
        let cids: Vec<_> = input.card_ids.into_iter().map(CardID).collect();
        self.with_col(|col| col.set_due_date(&cids, &days, config).map(Into::into))
    }

    fn sort_cards(&self, input: pb::SortCardsIn) -> Result<pb::OpChangesWithCount> {
        let cids: Vec<_> = input.card_ids.into_iter().map(CardID).collect();
        let (start, step, random, shift) = (
            input.starting_from,
            input.step_size,
            input.randomize,
            input.shift_existing,
        );
        let order = if random {
            NewCardSortOrder::Random
        } else {
            NewCardSortOrder::Preserve
        };
        self.with_col(|col| {
            col.sort_cards(&cids, start, step, order, shift)
                .map(Into::into)
        })
    }

    fn sort_deck(&self, input: pb::SortDeckIn) -> Result<pb::OpChangesWithCount> {
        self.with_col(|col| {
            col.sort_deck(input.deck_id.into(), input.randomize)
                .map(Into::into)
        })
    }

    fn get_next_card_states(&self, input: pb::CardId) -> Result<pb::NextCardStates> {
        let cid: CardID = input.into();
        self.with_col(|col| col.get_next_card_states(cid))
            .map(Into::into)
    }

    fn describe_next_states(&self, input: pb::NextCardStates) -> Result<pb::StringList> {
        let states: NextCardStates = input.into();
        self.with_col(|col| col.describe_next_states(states))
            .map(Into::into)
    }

    fn state_is_leech(&self, input: pb::SchedulingState) -> Result<pb::Bool> {
        let state: CardState = input.into();
        Ok(state.leeched().into())
    }

    fn answer_card(&self, input: pb::AnswerCardIn) -> Result<pb::OpChanges> {
        self.with_col(|col| col.answer_card(&input.into()))
            .map(Into::into)
    }

    fn upgrade_scheduler(&self, _input: pb::Empty) -> Result<pb::Empty> {
        self.with_col(|col| col.transact_no_undo(|col| col.upgrade_to_v2_scheduler()))
            .map(Into::into)
    }

    fn get_queued_cards(&self, input: pb::GetQueuedCardsIn) -> Result<pb::GetQueuedCardsOut> {
        self.with_col(|col| col.get_queued_cards(input.fetch_limit, input.intraday_learning_only))
    }
}

impl From<crate::scheduler::timing::SchedTimingToday> for pb::SchedTimingTodayOut {
    fn from(t: crate::scheduler::timing::SchedTimingToday) -> pb::SchedTimingTodayOut {
        pb::SchedTimingTodayOut {
            days_elapsed: t.days_elapsed,
            next_day_at: t.next_day_at,
        }
    }
}
