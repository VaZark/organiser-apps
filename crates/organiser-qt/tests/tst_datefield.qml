import QtQuick
import QtTest

Item {
    width: 400; height: 100

    // German writes dates dd.MM.yyyy; the stored date is ISO whatever the locale.
    DateField {
        id: f
        width: 300
        locale: Qt.locale("de_DE")
        text: "2026-11-30"
        property var log: []
        onCommitted: iso => log.push(iso)
    }
    LocaleDates { id: us; locale: Qt.locale("en_US") }
    LocaleDates { id: gb; locale: Qt.locale("en_GB") }
    LocaleDates { id: de; locale: Qt.locale("de_DE") }

    TestCase {
        name: "DateField"
        when: windowShown

        function test_the_locale_decides_how_a_date_is_written_and_read() {
            compare(us.show("2026-11-30"), "11/30/2026")
            compare(gb.show("2026-11-30"), "30/11/2026")
            compare(de.show("2026-11-30"), "30.11.2026")
            compare(us.parseInput("11/30/2026"), "2026-11-30")
            compare(gb.parseInput("30/11/2026"), "2026-11-30")
            compare(de.parseInput("30.11.2026"), "2026-11-30")
            compare(de.parseInput(""), "")
        }
        function test_not_a_real_date_is_refused() {
            compare(de.parseInput("31.02.2026"), null)
            compare(de.parseInput("2026-11-30"), null) // ISO is not what a German types
            compare(de.parseInput("tomorrow"), null)
            compare(de.parseInput("30.11."), null)
            compare(de.show(""), "")
            compare(de.show("nonsense"), "")
        }
        function test_the_field_shows_the_locale_and_stores_iso() {
            const input = f.children[0]
            compare(input.text, "30.11.2026")
            f.log = []
            input.forceActiveFocus()
            input.text = "31.02.2026"
            input.editingFinished()
            compare(f.log.length, 0)
            compare(input.text, "30.11.2026") // refused: old date back
            input.text = "01.12.2026"
            input.editingFinished()
            compare(f.log, ["2026-12-01"])
            compare(f.text, "2026-12-01")
            input.text = ""
            input.editingFinished()
            compare(f.log, ["2026-12-01", ""])
        }
    }
}
