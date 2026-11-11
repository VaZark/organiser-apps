//! Quantities and units. Amounts are compared in a base unit per kind of thing (grams for mass,
//! millilitres for volume, pieces for counts), so 1 kg and 500 g add up; a unit that is none of
//! those (`can`, `bunch`) only matches the same word.

/// What a unit measures.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Family {
    Mass,
    Volume,
    Count,
    /// Any other unit, by its (lowercase) word.
    Other(String),
}

/// `(family, how many base units one of it is)`.
pub fn parse(unit: &str) -> (Family, f64) {
    let u = unit.trim().trim_end_matches('.').to_lowercase();
    match u.as_str() {
        "g" | "gram" | "grams" | "gr" => (Family::Mass, 1.0),
        "kg" | "kilo" | "kilos" | "kilogram" | "kilograms" => (Family::Mass, 1000.0),
        "oz" | "ounce" | "ounces" => (Family::Mass, 28.3495),
        "lb" | "lbs" | "pound" | "pounds" => (Family::Mass, 453.592),
        "ml" | "millilitre" | "milliliter" | "millilitres" | "milliliters" => (Family::Volume, 1.0),
        "cl" => (Family::Volume, 10.0),
        "dl" => (Family::Volume, 100.0),
        "l" | "litre" | "liter" | "litres" | "liters" => (Family::Volume, 1000.0),
        "tsp" | "teaspoon" | "teaspoons" => (Family::Volume, 4.92892),
        "tbsp" | "tablespoon" | "tablespoons" => (Family::Volume, 14.7868),
        "cup" | "cups" => (Family::Volume, 236.588),
        "" | "pc" | "pcs" | "piece" | "pieces" | "x" | "whole" | "clove" | "cloves" | "slice"
        | "slices" => (Family::Count, 1.0),
        _ => (Family::Other(u), 1.0),
    }
}

/// An amount in the base unit of its family.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    pub amount: f64,
    pub family: Family,
}

pub fn to_base(amount: f64, unit: &str) -> Measure {
    let (family, factor) = parse(unit);
    Measure {
        amount: amount * factor,
        family,
    }
}

/// A base amount shown the way a person reads it: in `prefer` (the unit the recipe used) when that
/// is the same kind of thing, else in grams, millilitres or pieces, moving up to kilograms or
/// litres from a thousand.
pub fn from_base(amount: f64, family: &Family, prefer: &str) -> (f64, String) {
    let (pf, factor) = parse(prefer);
    if &pf == family {
        return (amount / factor, prefer.trim().to_string());
    }
    match family {
        Family::Mass if amount >= 1000.0 => (amount / 1000.0, "kg".into()),
        Family::Mass => (amount, "g".into()),
        Family::Volume if amount >= 1000.0 => (amount / 1000.0, "l".into()),
        Family::Volume => (amount, "ml".into()),
        Family::Count => (amount, String::new()),
        Family::Other(u) => (amount, u.clone()),
    }
}

/// `2`, `1.5`, `0.33`: no trailing zeros, at most two decimals.
pub fn pretty(amount: f64) -> String {
    let r = (amount * 100.0).round() / 100.0;
    let s = format!("{r:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_of_one_kind_share_a_base_so_they_add_up() {
        assert_eq!(to_base(1.0, "kg"), to_base(1000.0, "g"));
        assert_eq!(to_base(2.0, "L").amount, 2000.0);
        assert_eq!(to_base(3.0, "tbsp").family, Family::Volume);
        assert!((to_base(1.0, "cup").amount - 236.588).abs() < 1e-9);
        assert_eq!(to_base(3.0, "").family, Family::Count);
        assert_eq!(to_base(3.0, "Cloves").family, Family::Count);
    }

    #[test]
    fn an_unknown_unit_only_matches_itself() {
        assert_eq!(parse("Can."), (Family::Other("can".into()), 1.0));
        assert_ne!(parse("can").0, parse("bunch").0);
        assert_ne!(parse("can").0, Family::Count);
    }

    #[test]
    fn amounts_come_back_in_the_preferred_unit_when_it_fits_or_a_sensible_one_when_not() {
        assert_eq!(from_base(1500.0, &Family::Mass, "g"), (1500.0, "g".into()));
        assert_eq!(from_base(1500.0, &Family::Mass, "kg"), (1.5, "kg".into()));
        assert_eq!(
            from_base(1500.0, &Family::Mass, ""),
            (1.5, "kg".into()),
            "no preference: moves up"
        );
        assert_eq!(from_base(250.0, &Family::Volume, "tbsp").1, "tbsp");
        assert_eq!(
            from_base(250.0, &Family::Volume, "kg"),
            (250.0, "ml".into()),
            "a mass unit does not fit"
        );
        assert_eq!(from_base(3.0, &Family::Count, "pcs"), (3.0, "pcs".into()));
        assert_eq!(
            from_base(2.0, &Family::Other("can".into()), "can"),
            (2.0, "can".into())
        );
    }

    #[test]
    fn amounts_read_cleanly() {
        assert_eq!(pretty(2.0), "2");
        assert_eq!(pretty(1.5), "1.5");
        assert_eq!(pretty(0.333333), "0.33");
        assert_eq!(pretty(10.0), "10");
        assert_eq!(pretty(0.004), "0");
    }
}
