import QtQml

/// Dates as the person's system locale writes them. What is stored (files, database, backends) is
/// always ISO `YYYY-MM-DD`; this is only for showing and typing. The short format of the locale
/// (`29/11/2026`, `11/29/2026`, `29.11.2026`) with a four-digit year, so a typed date is not
/// ambiguous.
QtObject {
    id: dates

    /// The locale to use; the system's unless a test sets another.
    property var locale: Qt.locale()
    /// The format to show and type, e.g. `dd/MM/yyyy`.
    readonly property string format: {
        const f = dates.locale.dateFormat(Locale.ShortFormat)
        return f.indexOf("yyyy") >= 0 ? f : f.replace("yy", "yyyy")
    }

    /// A `Date` from an ISO string, or null when it is empty or not a date.
    function fromIso(iso) {
        const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso ? iso : "")
        if (!m) return null
        const d = new Date(+m[1], +m[2] - 1, +m[3])
        return d.getFullYear() === +m[1] && d.getMonth() === +m[2] - 1 && d.getDate() === +m[3] ? d : null
    }

    /// ISO text of a `Date`.
    function toIso(d) {
        const p = n => (n < 10 ? "0" : "") + n
        return d.getFullYear() + "-" + p(d.getMonth() + 1) + "-" + p(d.getDate())
    }

    /// An ISO date as the locale writes it ("" for none or for something that is not a date).
    function show(iso) {
        const d = dates.fromIso(iso)
        return d ? Qt.formatDate(d, dates.format) : ""
    }

    /// What was typed in the locale's format, as ISO: "" for empty, null when it is not a real date.
    function parseInput(text) {
        const t = text ? text.trim() : ""
        if (t === "") return ""
        const d = Date.fromLocaleDateString(dates.locale, t, dates.format)
        if (isNaN(d.valueOf()) || d.getFullYear() < 1000 || d.getFullYear() > 9999) return null
        return dates.toIso(d)
    }
}
