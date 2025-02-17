# Copyright: Ankitects Pty Ltd and contributors
# License: GNU AGPL, version 3 or later; http://www.gnu.org/licenses/agpl.html

from typing import Callable, List, Optional

import aqt.deckchooser
import aqt.editor
import aqt.forms
from anki.collection import OpChanges, SearchNode
from anki.consts import MODEL_CLOZE
from anki.notes import DuplicateOrEmptyResult, Note
from anki.utils import htmlToTextLine, isMac
from aqt import AnkiQt, gui_hooks
from aqt.note_ops import add_note
from aqt.notetypechooser import NoteTypeChooser
from aqt.qt import *
from aqt.sound import av_player
from aqt.utils import (
    TR,
    HelpPage,
    addCloseShortcut,
    askUser,
    disable_help_button,
    downArrow,
    openHelp,
    restoreGeom,
    saveGeom,
    shortcut,
    showWarning,
    tooltip,
    tr,
)


class AddCards(QDialog):
    def __init__(self, mw: AnkiQt) -> None:
        QDialog.__init__(self, None, Qt.Window)
        mw.garbage_collect_on_dialog_finish(self)
        self.mw = mw
        self.form = aqt.forms.addcards.Ui_Dialog()
        self.form.setupUi(self)
        self.setWindowTitle(tr(TR.ACTIONS_ADD))
        disable_help_button(self)
        self.setMinimumHeight(300)
        self.setMinimumWidth(400)
        self.setup_choosers()
        self.setupEditor()
        self.setupButtons()
        self._load_new_note()
        self.history: List[int] = []
        self._last_added_note: Optional[Note] = None
        restoreGeom(self, "add")
        addCloseShortcut(self)
        gui_hooks.add_cards_did_init(self)
        self.show()

    def setupEditor(self) -> None:
        self.editor = aqt.editor.Editor(self.mw, self.form.fieldsArea, self, True)

    def setup_choosers(self) -> None:
        defaults = self.mw.col.defaults_for_adding(
            current_review_card=self.mw.reviewer.card
        )
        self.notetype_chooser = NoteTypeChooser(
            mw=self.mw,
            widget=self.form.modelArea,
            starting_notetype_id=defaults.notetype_id,
            on_button_activated=self.show_notetype_selector,
            on_notetype_changed=self.on_notetype_change,
        )
        self.deck_chooser = aqt.deckchooser.DeckChooser(
            self.mw, self.form.deckArea, starting_deck_id=defaults.deck_id
        )

    def helpRequested(self) -> None:
        openHelp(HelpPage.ADDING_CARD_AND_NOTE)

    def setupButtons(self) -> None:
        bb = self.form.buttonBox
        ar = QDialogButtonBox.ActionRole
        # add
        self.addButton = bb.addButton(tr(TR.ACTIONS_ADD), ar)
        qconnect(self.addButton.clicked, self.add_current_note)
        self.addButton.setShortcut(QKeySequence("Ctrl+Return"))
        self.addButton.setToolTip(shortcut(tr(TR.ADDING_ADD_SHORTCUT_CTRLANDENTER)))
        # close
        self.closeButton = QPushButton(tr(TR.ACTIONS_CLOSE))
        self.closeButton.setAutoDefault(False)
        bb.addButton(self.closeButton, QDialogButtonBox.RejectRole)
        # help
        self.helpButton = QPushButton(tr(TR.ACTIONS_HELP), clicked=self.helpRequested)  # type: ignore
        self.helpButton.setAutoDefault(False)
        bb.addButton(self.helpButton, QDialogButtonBox.HelpRole)
        # history
        b = bb.addButton(f"{tr(TR.ADDING_HISTORY)} {downArrow()}", ar)
        if isMac:
            sc = "Ctrl+Shift+H"
        else:
            sc = "Ctrl+H"
        b.setShortcut(QKeySequence(sc))
        b.setToolTip(tr(TR.ADDING_SHORTCUT, val=shortcut(sc)))
        qconnect(b.clicked, self.onHistory)
        b.setEnabled(False)
        self.historyButton = b

    def setAndFocusNote(self, note: Note) -> None:
        self.editor.set_note(note, focusTo=0)

    def show_notetype_selector(self) -> None:
        self.editor.call_after_note_saved(self.notetype_chooser.choose_notetype)

    def on_notetype_change(self, notetype_id: int) -> None:
        # need to adjust current deck?
        if deck_id := self.mw.col.default_deck_for_notetype(notetype_id):
            self.deck_chooser.selected_deck_id = deck_id

        # only used for detecting changed sticky fields on close
        self._last_added_note = None

        # copy fields into new note with the new notetype
        old = self.editor.note
        new = self._new_note()
        if old:
            old_fields = list(old.keys())
            new_fields = list(new.keys())
            for n, f in enumerate(new.model()["flds"]):
                field_name = f["name"]
                # copy identical fields
                if field_name in old_fields:
                    new[field_name] = old[field_name]
                elif n < len(old.model()["flds"]):
                    # set non-identical fields by field index
                    old_field_name = old.model()["flds"][n]["name"]
                    if old_field_name not in new_fields:
                        new.fields[n] = old.fields[n]

        # and update editor state
        self.editor.note = new
        self.editor.loadNote()

    def _load_new_note(self, sticky_fields_from: Optional[Note] = None) -> None:
        note = self._new_note()
        if old_note := sticky_fields_from:
            flds = note.model()["flds"]
            # copy fields from old note
            if old_note:
                for n in range(min(len(note.fields), len(old_note.fields))):
                    if flds[n]["sticky"]:
                        note.fields[n] = old_note.fields[n]
        self.setAndFocusNote(note)

    def _new_note(self) -> Note:
        return self.mw.col.new_note(
            self.mw.col.models.get(self.notetype_chooser.selected_notetype_id)
        )

    def addHistory(self, note: Note) -> None:
        self.history.insert(0, note.id)
        self.history = self.history[:15]
        self.historyButton.setEnabled(True)

    def onHistory(self) -> None:
        m = QMenu(self)
        for nid in self.history:
            if self.mw.col.findNotes(SearchNode(nid=nid)):
                note = self.mw.col.get_note(nid)
                fields = note.fields
                txt = htmlToTextLine(", ".join(fields))
                if len(txt) > 30:
                    txt = f"{txt[:30]}..."
                line = tr(TR.ADDING_EDIT, val=txt)
                line = gui_hooks.addcards_will_add_history_entry(line, note)
                a = m.addAction(line)
                qconnect(a.triggered, lambda b, nid=nid: self.editHistory(nid))
            else:
                a = m.addAction(tr(TR.ADDING_NOTE_DELETED))
                a.setEnabled(False)
        gui_hooks.add_cards_will_show_history_menu(self, m)
        m.exec_(self.historyButton.mapToGlobal(QPoint(0, 0)))

    def editHistory(self, nid: int) -> None:
        aqt.dialogs.open("Browser", self.mw, search=(SearchNode(nid=nid),))

    def add_current_note(self) -> None:
        self.editor.call_after_note_saved(self._add_current_note)

    def _add_current_note(self) -> None:
        note = self.editor.note

        if not self._note_can_be_added(note):
            return

        target_deck_id = self.deck_chooser.selected_deck_id

        def on_success(changes: OpChanges) -> None:
            # only used for detecting changed sticky fields on close
            self._last_added_note = note

            self.addHistory(note)

            # workaround for PyQt focus bug
            self.editor.hideCompleters()

            tooltip(tr(TR.ADDING_ADDED), period=500)
            av_player.stop_and_clear_queue()
            self._load_new_note(sticky_fields_from=note)
            gui_hooks.add_cards_did_add_note(note)

        add_note(
            mw=self.mw, note=note, target_deck_id=target_deck_id, success=on_success
        )

    def _note_can_be_added(self, note: Note) -> bool:
        result = note.duplicate_or_empty()
        if result == DuplicateOrEmptyResult.EMPTY:
            problem = tr(TR.ADDING_THE_FIRST_FIELD_IS_EMPTY)
        else:
            # duplicate entries are allowed these days
            problem = None

        # filter problem through add-ons
        problem = gui_hooks.add_cards_will_add_note(problem, note)
        if problem is not None:
            showWarning(problem, help=HelpPage.ADDING_CARD_AND_NOTE)
            return False

        # missing cloze deletion?
        if note.model()["type"] == MODEL_CLOZE:
            if not note.cloze_numbers_in_fields():
                if not askUser(tr(TR.ADDING_YOU_HAVE_A_CLOZE_DELETION_NOTE)):
                    return False

        return True

    def keyPressEvent(self, evt: QKeyEvent) -> None:
        "Show answer on RET or register answer."
        if evt.key() in (Qt.Key_Enter, Qt.Key_Return) and self.editor.tags.hasFocus():
            evt.accept()
            return
        return QDialog.keyPressEvent(self, evt)

    def reject(self) -> None:
        self.ifCanClose(self._reject)

    def _reject(self) -> None:
        av_player.stop_and_clear_queue()
        self.editor.cleanup()
        self.notetype_chooser.cleanup()
        self.mw.maybeReset()
        saveGeom(self, "add")
        aqt.dialogs.markClosed("AddCards")
        QDialog.reject(self)

    def ifCanClose(self, onOk: Callable) -> None:
        def afterSave() -> None:
            ok = self.editor.fieldsAreBlank(self._last_added_note) or askUser(
                tr(TR.ADDING_CLOSE_AND_LOSE_CURRENT_INPUT), defaultno=True
            )
            if ok:
                onOk()

        self.editor.call_after_note_saved(afterSave)

    def closeWithCallback(self, cb: Callable[[], None]) -> None:
        def doClose() -> None:
            self._reject()
            cb()

        self.ifCanClose(doClose)

    # legacy aliases

    addCards = add_current_note
    _addCards = _add_current_note
    onModelChange = on_notetype_change

    def addNote(self, note: Note) -> None:
        print("addNote() is obsolete")

    def removeTempNote(self, note: Note) -> None:
        print("removeTempNote() will go away")
