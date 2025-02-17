# Copyright: Ankitects Pty Ltd and contributors
# License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

from __future__ import annotations

from typing import Sequence

from aqt import AnkiQt


def set_card_deck(*, mw: AnkiQt, card_ids: Sequence[int], deck_id: int) -> None:
    mw.perform_op(lambda: mw.col.set_deck(card_ids, deck_id))


def set_card_flag(*, mw: AnkiQt, card_ids: Sequence[int], flag: int) -> None:
    mw.perform_op(lambda: mw.col.set_user_flag_for_cards(flag, card_ids))
