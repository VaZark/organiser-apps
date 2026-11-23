//! What is missing. A recipe needs ingredients; the fridge has some; for one recipe times a number
//! (five portions of meal prep) or for everything planned in a week, this says what to buy and how
//! much. Pure: the database only supplies the lists.
//!
//! Ingredients are matched by name (case and plural ignored) and compared in a common unit (see
//! [`units`](super::units)). An ingredient with no quantity ("salt, to taste") only has to be
//! there; something in the fridge with no quantity counts as enough. An ingredient the recipe marks
//! optional is never missing.

use std::collections::HashMap;

use chrono::NaiveDate;

use super::model::{Ingredient, Stock};
use super::units::{self, Family};

/// `Tomatoes `, `tomato` and `TOMATO` are one ingredient.
pub fn normalize(name: &str) -> String {
    let n = name
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if n.len() > 3 && n.ends_with("ies") {
        format!("{}y", &n[..n.len() - 3])
    } else if n.len() > 3 && (n.ends_with("oes") || n.ends_with("ches") || n.ends_with("shes")) {
        n[..n.len() - 2].to_string()
    } else if n.len() > 2 && n.ends_with('s') && !n.ends_with("ss") {
        n[..n.len() - 1].to_string()
    } else {
        n
    }
}

/// An amount of one ingredient that is wanted or on hand, in its family's base unit.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// The name as first written, for showing.
    pub name: String,
    pub key: String,
    pub family: Family,
    /// `None`: no quantity (to taste; or in the fridge, "some").
    pub amount: Option<f64>,
    /// The unit as written, to show an amount back in it.
    pub unit: String,
    pub optional: bool,
}

/// What `ingredients` call for when the recipe is made `times` over.
pub fn needs(ingredients: &[Ingredient], times: f64) -> Vec<Line> {
    ingredients
        .iter()
        .map(|i| {
            let measure = i.quantity.map(|q| units::to_base(q * times, &i.unit));
            Line {
                name: i.name.trim().to_string(),
                key: normalize(&i.name),
                family: measure
                    .as_ref()
                    .map_or_else(|| units::parse(&i.unit).0, |m| m.family.clone()),
                amount: measure.map(|m| m.amount),
                unit: i.unit.clone(),
                optional: i.optional,
            }
        })
        .collect()
}

/// What is in the fridge and still good on `today`.
pub fn haves(stock: &[Stock], today: NaiveDate) -> Vec<Line> {
    stock
        .iter()
        .filter(|s| !s.is_expired(today))
        .map(|s| {
            let measure = s.quantity.map(|q| units::to_base(q, &s.unit));
            Line {
                name: s.name.trim().to_string(),
                key: normalize(&s.name),
                family: measure
                    .as_ref()
                    .map_or_else(|| units::parse(&s.unit).0, |m| m.family.clone()),
                amount: measure.map(|m| m.amount),
                unit: s.unit.clone(),
                optional: false,
            }
        })
        .collect()
}

/// Lines of one ingredient and kind of unit added into one, first seen first. A line with no
/// quantity next to ones with a quantity adds nothing to the sum; it is optional only if every
/// line was.
pub fn combine(lines: Vec<Line>) -> Vec<Line> {
    let mut out: Vec<Line> = Vec::new();
    let mut at: HashMap<(String, Family), usize> = HashMap::new();
    for l in lines {
        match at.get(&(l.key.clone(), l.family.clone())) {
            Some(&i) => {
                let t = &mut out[i];
                t.amount = match (t.amount, l.amount) {
                    (Some(a), Some(b)) => Some(a + b),
                    (a, b) => a.or(b),
                };
                t.optional &= l.optional;
            }
            None => {
                at.insert((l.key.clone(), l.family.clone()), out.len());
                out.push(l);
            }
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    /// The fridge has enough (or, with no quantity wanted, has it).
    Enough,
    /// Not in the fridge at all.
    Missing,
    /// In the fridge, but less than wanted.
    Short,
    /// In the fridge in a unit that cannot be compared with the one wanted.
    Unit,
}

/// One ingredient wanted, set against the fridge.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub status: Status,
    /// The amount wanted, as written units (`None`: no quantity).
    pub need: Option<(f64, String)>,
    /// How much to get: what is wanted less what there is (`None` when it is "get some").
    pub get: Option<(f64, String)>,
    pub optional: bool,
}

/// Every wanted ingredient with its status. `include_optional` keeps the optional ones in.
pub fn compare(wanted: &[Line], fridge: &[Line], include_optional: bool) -> Vec<Item> {
    let wanted = combine(wanted.to_vec());
    let fridge = combine(fridge.to_vec());
    wanted
        .iter()
        .filter(|w| include_optional || !w.optional)
        .map(|w| {
            let shown = |base: f64| units::from_base(base, &w.family, &w.unit);
            // The same kind of unit; failing that, a line with no quantity ("some") is enough
            // in any unit.
            let same = fridge
                .iter()
                .find(|f| f.key == w.key && f.family == w.family)
                .or_else(|| fridge.iter().find(|f| f.key == w.key && f.amount.is_none()));
            let other = fridge.iter().any(|f| f.key == w.key);
            let (status, get) = match (same, w.amount) {
                // Something of it, no amount asked for or no amount known: enough.
                (Some(_), None) => (Status::Enough, None),
                (Some(f), Some(_)) if f.amount.is_none() => (Status::Enough, None),
                (Some(f), Some(need)) => {
                    let have = f.amount.unwrap_or(0.0);
                    if have + 1e-9 >= need {
                        (Status::Enough, None)
                    } else {
                        (Status::Short, Some(shown(need - have)))
                    }
                }
                (None, None) if other => (Status::Enough, None),
                (None, Some(need)) if other => (Status::Unit, Some(shown(need))),
                (None, Some(need)) => (Status::Missing, Some(shown(need))),
                (None, None) => (Status::Missing, None),
            };
            Item {
                name: w.name.clone(),
                status,
                need: w.amount.map(shown),
                get,
                optional: w.optional,
            }
        })
        .collect()
}

/// What to get: the ingredients that are not [`Status::Enough`].
pub fn to_get(items: Vec<Item>) -> Vec<Item> {
    items
        .into_iter()
        .filter(|i| i.status != Status::Enough)
        .collect()
}

/// Whether everything the recipe needs (not counting optional ingredients) is in the fridge.
pub fn can_make(items: &[Item]) -> bool {
    items
        .iter()
        .all(|i| i.optional || i.status == Status::Enough)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ing(name: &str, q: Option<f64>, unit: &str, optional: bool) -> Ingredient {
        Ingredient {
            id: 0,
            recipe_id: 1,
            name: name.into(),
            quantity: q,
            unit: unit.into(),
            optional,
            position: 0,
        }
    }

    fn stock(name: &str, q: Option<f64>, unit: &str) -> Stock {
        Stock {
            id: 0,
            name: name.into(),
            quantity: q,
            unit: unit.into(),
            expires: None,
        }
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
    }

    fn get(wanted: Vec<Line>, fridge: Vec<Line>) -> Vec<Item> {
        to_get(compare(&wanted, &fridge, false))
    }

    #[test]
    fn names_match_ignoring_case_spacing_and_plurals() {
        for same in ["Tomatoes", " tomato ", "TOMATO", "tomatoes"] {
            assert_eq!(normalize(same), "tomato", "{same}");
        }
        assert_eq!(normalize("Cherry  Tomatoes"), "cherry tomato");
        assert_eq!(normalize("berries"), "berry");
        assert_eq!(normalize("peaches"), "peach");
        assert_eq!(normalize("grass"), "grass", "double s stays");
        assert_eq!(normalize("rice"), "rice");
        assert_eq!(
            normalize("gas"),
            "ga".to_string() + "",
            "short words lose only a final s"
        );
    }

    #[test]
    fn a_recipe_made_several_times_over_needs_several_times_the_amounts() {
        let w = needs(
            &[
                ing("flour", Some(200.0), "g", false),
                ing("salt", None, "", false),
            ],
            5.0,
        );
        assert_eq!(w[0].amount, Some(1000.0));
        assert_eq!(w[1].amount, None, "to taste stays to taste");
    }

    #[test]
    fn what_the_fridge_covers_is_not_missing_and_the_rest_is_listed_with_how_much() {
        let wanted = needs(
            &[
                ing("flour", Some(500.0), "g", false),
                ing("milk", Some(0.5), "l", false),
                ing("eggs", Some(3.0), "", false),
            ],
            1.0,
        );
        let fridge = haves(
            &[
                stock("Flour", Some(1.0), "kg"),
                stock("milk", Some(200.0), "ml"),
            ],
            today(),
        );
        let g = get(wanted, fridge);
        assert_eq!(g.len(), 2, "flour is covered");
        assert_eq!(
            (g[0].name.as_str(), g[0].status.clone()),
            ("milk", Status::Short)
        );
        let (amount, unit) = g[0].get.clone().unwrap();
        assert!(
            (amount - 0.3).abs() < 1e-6 && unit == "l",
            "300 ml short, shown in litres as the recipe wrote"
        );
        assert_eq!(
            (g[1].name.as_str(), g[1].status.clone()),
            ("eggs", Status::Missing)
        );
        assert_eq!(g[1].get, Some((3.0, String::new())));
    }

    #[test]
    fn the_same_ingredient_across_recipes_adds_up_before_the_fridge_is_taken_off() {
        // 300 g + 0.4 kg = 700 g wanted; 500 g in the fridge: 200 g to get.
        let mut wanted = needs(&[ing("rice", Some(300.0), "g", false)], 1.0);
        wanted.extend(needs(&[ing("Rice", Some(0.4), "kg", false)], 1.0));
        let g = get(wanted, haves(&[stock("rice", Some(500.0), "g")], today()));
        assert_eq!(g.len(), 1);
        let (amount, unit) = g[0].get.clone().unwrap();
        assert!((amount - 200.0).abs() < 1e-6, "{amount}");
        assert_eq!(unit, "g");
    }

    #[test]
    fn an_ingredient_with_no_quantity_only_needs_to_be_there_and_a_fridge_item_with_none_is_enough()
    {
        let salt = needs(&[ing("salt", None, "", false)], 1.0);
        assert!(get(salt.clone(), haves(&[stock("salt", None, "")], today())).is_empty());
        assert!(
            get(
                salt.clone(),
                haves(&[stock("salt", Some(500.0), "g")], today())
            )
            .is_empty(),
            "present in some unit"
        );
        let g = get(salt, vec![]);
        assert_eq!(
            (g[0].status.clone(), g[0].get.clone()),
            (Status::Missing, None),
            "get some"
        );
        // "some flour" in the fridge covers 500 g of it.
        assert!(
            get(
                needs(&[ing("flour", Some(500.0), "g", false)], 1.0),
                haves(&[stock("flour", None, "")], today())
            )
            .is_empty()
        );
    }

    #[test]
    fn expired_fridge_items_do_not_count_as_something_you_have() {
        let mut old = stock("milk", Some(1.0), "l");
        old.expires = NaiveDate::from_ymd_opt(2026, 11, 10);
        let mut fresh = stock("eggs", Some(6.0), "");
        fresh.expires = Some(today()); // good through the day itself
        let have = haves(&[old, fresh], today());
        assert_eq!(have.len(), 1);
        let wanted = needs(
            &[
                ing("milk", Some(0.5), "l", false),
                ing("eggs", Some(2.0), "", false),
            ],
            1.0,
        );
        let g = get(wanted, have);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "milk");
    }

    #[test]
    fn units_that_cannot_be_compared_are_flagged_not_guessed() {
        // 2 onions wanted, 500 g in the fridge: cannot say.
        let g = get(
            needs(&[ing("onion", Some(2.0), "", false)], 1.0),
            haves(&[stock("onions", Some(500.0), "g")], today()),
        );
        assert_eq!(g[0].status, Status::Unit);
        assert_eq!(g[0].get, Some((2.0, String::new())));
    }

    #[test]
    fn optional_ingredients_are_never_missing_unless_asked_for() {
        let w = needs(
            &[
                ing("parsley", Some(1.0), "bunch", true),
                ing("pasta", Some(200.0), "g", false),
            ],
            1.0,
        );
        assert_eq!(get(w.clone(), vec![]).len(), 1, "only the pasta");
        assert_eq!(compare(&w, &[], true).len(), 2);
        assert!(!can_make(&compare(&w, &[], false)));
        assert!(
            can_make(&compare(
                &w,
                &haves(&[stock("pasta", Some(1.0), "kg")], today()),
                false
            )),
            "parsley is optional"
        );
    }

    #[test]
    fn a_week_of_meal_prep_is_one_shopping_list() {
        // Five days of the same lunch, plus a dinner that shares the onions.
        let lunch = [
            ing("chicken", Some(150.0), "g", false),
            ing("onion", Some(1.0), "", false),
        ];
        let dinner = [
            ing("onion", Some(2.0), "", false),
            ing("cream", Some(1.0), "dl", false),
        ];
        let mut wanted = needs(&lunch, 5.0);
        wanted.extend(needs(&dinner, 1.0));
        let g = get(
            wanted,
            haves(
                &[
                    stock("onions", Some(4.0), ""),
                    stock("chicken", Some(0.5), "kg"),
                ],
                today(),
            ),
        );
        let find = |n: &str| {
            g.iter()
                .find(|i| i.name == n)
                .map(|i| i.get.clone().unwrap())
        };
        assert_eq!(
            find("onion"),
            Some((3.0, String::new())),
            "7 wanted, 4 on hand"
        );
        let (c, unit) = find("chicken").unwrap();
        assert!(
            (c - 250.0).abs() < 1e-6 && unit == "g",
            "750 g wanted, 500 g on hand"
        );
        assert_eq!(find("cream"), Some((1.0, "dl".into())));
    }
}
