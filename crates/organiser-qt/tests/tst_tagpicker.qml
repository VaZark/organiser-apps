import QtQuick
import QtQuick.Controls as QQC2
import QtTest

Item {
    id: root
    width: 420; height: 500

    TagPicker {
        id: picker
        width: 360
        allTags: [{name:"home"},{name:"work"},{name:"homework"},{name:"garden"},{name:"t1"},{name:"t2"},{name:"t3"},{name:"t4"},{name:"t5"},{name:"t6"},{name:"t7"},{name:"t8"},{name:"t9"}]
        property var log: []
        // The owner keeps the chosen list, as the filter sheets do.
        onToggled: name => {
            log = log.concat([name])
            picked = picked.indexOf(name) >= 0 ? picked.filter(t => t !== name) : picked.concat([name])
        }
    }

    function findDelegates(item, out) {
        for (const c of item.children) {
            if (c instanceof QQC2.ItemDelegate) out.push(c)
            findDelegates(c, out)
        }
        return out
    }
    function suggestionTexts() {
        return findDelegates(QQC2.Overlay.overlay, []).filter(d => d.visible).map(d => d.text)
    }

    TestCase {
        name: "TagPicker"
        when: windowShown

        function init() { picker.picked = []; picker.log = []; picker.children[1].text = "" }
        function field() { return picker.children[1] }
        function typeText(t) { for (const ch of t) keyClick(ch) }

        function test_1_nothing_listed_until_typing() {
            field().forceActiveFocus()
            compare(picker.matches.length, 0)
            compare(suggestionTexts().length, 0, "no tag is listed before typing")
        }

        function test_2_typing_filters_prefix_first_then_contains() {
            field().forceActiveFocus()
            typeText("ho")
            verify(picker.matches.map(t => t.name).indexOf("home") >= 0)
            verify(picker.matches.map(t => t.name).indexOf("homework") >= 0)
            compare(picker.matches.length, 2)
            wait(100)
            const shown = suggestionTexts()
            verify(shown.indexOf("home") >= 0 && shown.indexOf("homework") >= 0, "popup lists matches: " + shown)
            verify(shown.indexOf("garden") < 0, "non-matches are not listed")
        }

        function test_3_contains_match() {
            field().forceActiveFocus()
            typeText("ork")
            compare(picker.matches.map(t => t.name).sort(), ["homework", "work"])
        }

        function test_4_enter_picks_best_and_clears_field() {
            field().forceActiveFocus()
            typeText("wo")
            compare(picker.matches[0].name, "work", "prefix match first")
            keyClick(Qt.Key_Return)
            compare(picker.picked, ["work"])
            compare(field().text, "")
            compare(picker.matches.length, 0)
        }

        function test_5_multiselect_via_click_and_enter() {
            field().forceActiveFocus()
            typeText("ho"); keyClick(Qt.Key_Return)        // home (prefix, first)
            compare(picker.picked.length, 1)
            typeText("ho")
            // The chosen tag is not suggested again.
            verify(picker.matches.map(t => t.name).indexOf(picker.picked[0]) < 0)
            wait(100)
            const d = findDelegates(QQC2.Overlay.overlay, []).filter(x => x.visible && x.text !== "")
            verify(d.length > 0, "a suggestion is clickable")
            mouseClick(d[0])
            compare(picker.picked.length, 2, "second tag added by click")
            typeText("gar"); keyClick(Qt.Key_Return)
            compare(picker.picked.length, 3)
            compare(new Set(picker.picked).size, 3, "no duplicates")
            verify(picker.picked.indexOf("garden") >= 0)
        }

        function test_6_chip_remove_unselects() {
            picker.picked = ["home", "work"]
            wait(50)
            // Close buttons live in the chips; remove through the signal path the chip uses.
            picker.toggled("home")
            compare(picker.picked, ["work"])
        }

        function test_6b_clicking_a_chips_close_button_removes_the_tag() {
            picker.picked = ["home", "work"]
            wait(100)
            const chips = []
            const walk = it => { for (const c of it.children) { if (c.closable === true) chips.push(c); walk(c) } }
            walk(picker)
            compare(chips.length, 2)
            const chip = chips.find(c => c.text === "home")
            mouseClick(chip, chip.width - 8, chip.height / 2)
            compare(picker.picked, ["work"], "close click unselected the tag")
        }

        function test_7_escape_clears_text() {
            field().forceActiveFocus()
            typeText("ga")
            keyClick(Qt.Key_Escape)
            compare(field().text, "")
        }

        function test_8_at_most_eight_suggestions() {
            field().forceActiveFocus()
            typeText("t")      // t1..t9 and others containing t
            verify(picker.matches.length <= 8)
            compare(picker.matches.length, 8)
        }

        function test_9_enter_with_no_match_does_nothing() {
            field().forceActiveFocus()
            typeText("zzz"); keyClick(Qt.Key_Return)
            compare(picker.picked.length, 0)
        }
    }
}
