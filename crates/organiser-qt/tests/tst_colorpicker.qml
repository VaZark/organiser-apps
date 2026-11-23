import QtQuick
import QtQuick.Controls as QQC2
import QtTest

Item {
    width: 400; height: 200

    ColorPicker {
        id: p
        width: 360
        presets: [{name: "Red", hex: "#e66a6a"}, {name: "Blue", hex: "#6aa5e8"}]
        property var log: []
        onPicked: hex => { log.push(hex); color = hex }
    }

    function buttons(item, out) {
        for (const c of item.children) {
            if (c instanceof QQC2.AbstractButton || (c.clicked !== undefined)) out.push(c)
            buttons(c, out)
        }
        return out
    }

    TestCase {
        name: "ColorPicker"
        when: windowShown

        function test_swatch_none_and_typed() {
            const b = buttons(p, [])
            compare(b.length, 3) // two swatches and "No Color"
            compare(b[2].enabled, false) // no colour yet: nothing to clear
            b[1].clicked()
            compare(p.log, ["#6aa5e8"])
            compare(b[2].enabled, true)
            b[2].clicked()
            compare(p.log, ["#6aa5e8", ""])
            compare(p.color, "")
        }
        function test_typed_value_is_passed_on_for_the_backend_to_check() {
            p.log = []
            // The only TextField: type and accept.
            let tf = null
            const walk = it => { for (const c of it.children) { if (c instanceof QQC2.TextField) tf = c; walk(c) } }
            walk(p)
            tf.text = " #F80 "
            tf.accepted()
            compare(p.log, ["#F80"])
        }
    }
}
