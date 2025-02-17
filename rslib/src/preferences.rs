// Copyright: Ankitects Pty Ltd and contributors
// License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

use crate::{
    backend_proto::{
        preferences::scheduling::NewReviewMix as NewRevMixPB,
        preferences::{Editing, Reviewing, Scheduling},
        Preferences,
    },
    collection::Collection,
    config::BoolKey,
    err::Result,
    prelude::*,
    scheduler::timing::local_minutes_west_for_stamp,
};

impl Collection {
    pub fn get_preferences(&self) -> Result<Preferences> {
        Ok(Preferences {
            scheduling: Some(self.get_scheduling_preferences()?),
            reviewing: Some(self.get_reviewing_preferences()?),
            editing: Some(self.get_editing_preferences()?),
        })
    }

    pub fn set_preferences(&mut self, prefs: Preferences) -> Result<OpOutput<()>> {
        self.transact(Op::UpdatePreferences, |col| {
            col.set_preferences_inner(prefs)
        })
    }

    fn set_preferences_inner(&mut self, prefs: Preferences) -> Result<()> {
        if let Some(sched) = prefs.scheduling {
            self.set_scheduling_preferences(sched)?;
        }
        if let Some(reviewing) = prefs.reviewing {
            self.set_reviewing_preferences(reviewing)?;
        }
        if let Some(editing) = prefs.editing {
            self.set_editing_preferences(editing)?;
        }
        Ok(())
    }

    pub fn get_scheduling_preferences(&self) -> Result<Scheduling> {
        Ok(Scheduling {
            scheduler_version: match self.scheduler_version() {
                crate::config::SchedulerVersion::V1 => 1,
                crate::config::SchedulerVersion::V2 => 2,
            },
            rollover: self.rollover_for_current_scheduler()? as u32,
            learn_ahead_secs: self.learn_ahead_secs(),
            new_review_mix: match self.get_new_review_mix() {
                crate::config::NewReviewMix::Mix => NewRevMixPB::Distribute,
                crate::config::NewReviewMix::ReviewsFirst => NewRevMixPB::ReviewsFirst,
                crate::config::NewReviewMix::NewFirst => NewRevMixPB::NewFirst,
            } as i32,
            new_timezone: self.get_creation_utc_offset().is_some(),
            day_learn_first: self.get_bool(BoolKey::ShowDayLearningCardsFirst),
        })
    }

    pub(crate) fn set_scheduling_preferences(&mut self, settings: Scheduling) -> Result<()> {
        let s = settings;

        self.set_bool(BoolKey::ShowDayLearningCardsFirst, s.day_learn_first)?;
        self.set_learn_ahead_secs(s.learn_ahead_secs)?;

        self.set_new_review_mix(match s.new_review_mix() {
            NewRevMixPB::Distribute => crate::config::NewReviewMix::Mix,
            NewRevMixPB::NewFirst => crate::config::NewReviewMix::NewFirst,
            NewRevMixPB::ReviewsFirst => crate::config::NewReviewMix::ReviewsFirst,
        })?;

        let created = self.storage.creation_stamp()?;

        if self.rollover_for_current_scheduler()? != s.rollover as u8 {
            self.set_rollover_for_current_scheduler(s.rollover as u8)?;
        }

        if s.new_timezone {
            if self.get_creation_utc_offset().is_none() {
                self.set_creation_utc_offset(Some(local_minutes_west_for_stamp(created.0)))?;
            }
        } else {
            self.set_creation_utc_offset(None)?;
        }

        Ok(())
    }

    pub fn get_reviewing_preferences(&self) -> Result<Reviewing> {
        Ok(Reviewing {
            hide_audio_play_buttons: self.get_bool(BoolKey::HideAudioPlayButtons),
            interrupt_audio_when_answering: self.get_bool(BoolKey::InterruptAudioWhenAnswering),
            show_remaining_due_counts: self.get_bool(BoolKey::ShowRemainingDueCountsInStudy),
            show_intervals_on_buttons: self.get_bool(BoolKey::ShowIntervalsAboveAnswerButtons),
            time_limit_secs: self.get_answer_time_limit_secs(),
        })
    }

    pub(crate) fn set_reviewing_preferences(&mut self, settings: Reviewing) -> Result<()> {
        let s = settings;
        self.set_bool(BoolKey::HideAudioPlayButtons, s.hide_audio_play_buttons)?;
        self.set_bool(
            BoolKey::InterruptAudioWhenAnswering,
            s.interrupt_audio_when_answering,
        )?;
        self.set_bool(
            BoolKey::ShowRemainingDueCountsInStudy,
            s.show_remaining_due_counts,
        )?;
        self.set_bool(
            BoolKey::ShowIntervalsAboveAnswerButtons,
            s.show_intervals_on_buttons,
        )?;
        self.set_answer_time_limit_secs(s.time_limit_secs)?;
        Ok(())
    }

    pub fn get_editing_preferences(&self) -> Result<Editing> {
        Ok(Editing {
            adding_defaults_to_current_deck: self.get_bool(BoolKey::AddingDefaultsToCurrentDeck),
            paste_images_as_png: self.get_bool(BoolKey::PasteImagesAsPng),
            paste_strips_formatting: self.get_bool(BoolKey::PasteStripsFormatting),
        })
    }

    pub(crate) fn set_editing_preferences(&mut self, settings: Editing) -> Result<()> {
        let s = settings;
        self.set_bool(
            BoolKey::AddingDefaultsToCurrentDeck,
            s.adding_defaults_to_current_deck,
        )?;
        self.set_bool(BoolKey::PasteImagesAsPng, s.paste_images_as_png)?;
        self.set_bool(BoolKey::PasteStripsFormatting, s.paste_strips_formatting)?;
        Ok(())
    }
}
