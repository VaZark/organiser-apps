//! The messages of the recipe page's parts (ingredients, steps, photos), the calendar and the
//! fridge. Each goes through `recipes-schema`; the rows hold what was typed.

use chrono::Days;
use iced::Task;
use organiser_ui::iad::primitives::date::Date as IadDate;
use recipes_schema::lifecycle;
use recipes_schema::model::{IngredientEdit, RecipeEdit, StepEdit};
use recipes_schema::{photos, views};

use crate::app::{
    App, MealChoice, Message, RecipeChoice, ShopRow, StockRow, View, parse_date, parse_qty,
    qty_text,
};

impl App {
    /// What the calendar and the fridge page show, read when they are in view.
    pub(crate) fn reload_extras(&mut self) {
        self.recipe_choices = views::recipes(
            &self.db,
            &views::Filter {
                sort: views::Sort::Title,
                ..views::Filter::default()
            },
        )
        .unwrap_or_default()
        .into_iter()
        .map(|r| RecipeChoice {
            id: r.id,
            title: r.title,
        })
        .collect();
        match self.view {
            View::Calendar => {
                let (from, to) = (
                    chrono::NaiveDate::from_ymd_opt(2000, 1, 1).expect("a date"),
                    chrono::NaiveDate::from_ymd_opt(2100, 12, 31).expect("a date"),
                );
                self.meals = views::meals_between(&self.db, from, to).unwrap_or_default();
            }
            View::Fridge => {
                self.stock = views::stock(&self.db)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|s| StockRow {
                        id: s.id,
                        name: s.name,
                        qty: qty_text(s.quantity),
                        unit: s.unit,
                        expires: s.expires.map(|d| d.to_string()).unwrap_or_default(),
                        date: s.expires,
                    })
                    .collect();
                self.stock_order = organiser_ui::iad::primitives::reorder::ReorderState::live(
                    self.stock.iter().map(|s| s.id).collect(),
                );
            }
            _ => {}
        }
    }

    /// Work out the shopping list for the recipes picked.
    pub(crate) fn rebuild_shop(&mut self) {
        let picks: Vec<(i64, f64)> = self
            .shop
            .iter()
            .map(|r| (r.recipe.id, views::times_for(r.servings, r.portions)))
            .collect();
        self.shop_items =
            views::shopping_list(&self.db, &picks, self.today, false).unwrap_or_default();
        // Remembered: the picks (portions, not times) and the day they last changed.
        let saved: Vec<(i64, f64)> = self
            .shop
            .iter()
            .map(|r| (r.recipe.id, r.portions))
            .collect();
        if let Ok(list) = lifecycle::save_shopping(&mut self.db, &saved) {
            self.shop_changed = list.updated;
        }
    }

    /// Bring back the remembered shopping list.
    pub(crate) fn load_shop(&mut self) {
        let Ok(list) = views::shopping_picks(&self.db) else {
            return;
        };
        self.shop_changed = list.updated;
        self.shop = list
            .picks
            .into_iter()
            .filter_map(|(id, portions)| {
                let r = recipes_schema::model::Recipe::from(&self.db.get(id).ok()?);
                Some(ShopRow {
                    recipe: RecipeChoice { id, title: r.title },
                    portions,
                    servings: r.servings,
                })
            })
            .collect();
        let picks: Vec<(i64, f64)> = self
            .shop
            .iter()
            .map(|r| (r.recipe.id, views::times_for(r.servings, r.portions)))
            .collect();
        self.shop_items =
            views::shopping_list(&self.db, &picks, self.today, false).unwrap_or_default();
    }

    fn servings_of(&self, recipe: i64) -> Option<f64> {
        self.db
            .get(recipe)
            .ok()
            .and_then(|i| recipes_schema::model::Recipe::from(&i).servings)
    }

    fn fail_in_page(&mut self, e: impl std::fmt::Display) {
        let msg = e.to_string();
        self.with_details(|d| d.error = Some(msg));
    }

    /// The open recipe's id, if it is saved.
    fn open_id(&self) -> Option<i64> {
        self.details
            .as_ref()
            .filter(|d| !d.is_new())
            .map(|d| d.entry.id)
    }

    fn photo_list(&self, target: Option<i64>) -> Vec<String> {
        let Some(d) = &self.details else {
            return Vec::new();
        };
        match target {
            None => d.entry.photos.clone(),
            Some(s) => d
                .steps
                .iter()
                .find(|r| r.id == s)
                .map(|r| r.photos.clone())
                .unwrap_or_default(),
        }
    }

    fn save_photos(&mut self, target: Option<i64>, list: Vec<String>) {
        let Some(id) = self.open_id() else { return };
        let result = match target {
            None => lifecycle::edit_recipe(
                &mut self.db,
                id,
                &RecipeEdit {
                    photos: Some(list),
                    ..RecipeEdit::default()
                },
            )
            .map(|_| ()),
            Some(s) => lifecycle::edit_step(
                &mut self.db,
                s,
                &StepEdit {
                    photos: Some(list),
                    text: None,
                },
            )
            .map(|_| ()),
        };
        if let Err(e) = result {
            self.fail_in_page(e);
        }
        let _ = photos::gc(&self.db, &photos::dir());
        self.refresh_details();
        self.load_parts();
        self.reload();
    }

    pub(crate) fn more(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::DEdit(on) => {
                self.with_details(|d| d.edit = on);
            }
            // ---- ingredients ----
            Message::IngAdd => {
                let Some(id) = self.open_id() else {
                    return Task::none();
                };
                let Some(d) = &self.details else {
                    return Task::none();
                };
                let n = &d.new_ing;
                let Ok(q) = parse_qty(&n.qty) else {
                    self.fail_in_page("The quantity is not a number.");
                    return Task::none();
                };
                match lifecycle::add_ingredient(&mut self.db, id, &n.name, q, &n.unit, n.optional) {
                    Ok(_) => {
                        self.load_parts();
                        self.with_details(|d| d.new_ing = crate::app::IngRow::blank());
                    }
                    Err(e) => self.fail_in_page(e),
                }
            }
            Message::IngName(id, s) => {
                self.with_details(|d| ing(d, id).name = s.clone());
                if id != 0 && !s.trim().is_empty() {
                    self.edit_ing(
                        id,
                        IngredientEdit {
                            name: Some(s),
                            ..Default::default()
                        },
                    );
                }
            }
            Message::IngQty(id, s) => {
                self.with_details(|d| ing(d, id).qty = s.clone());
                if id != 0 {
                    match parse_qty(&s) {
                        Ok(q) => self.edit_ing(
                            id,
                            IngredientEdit {
                                quantity: Some(q),
                                ..Default::default()
                            },
                        ),
                        Err(()) => self.fail_in_page("The quantity is not a number."),
                    }
                }
            }
            Message::IngUnit(id, s) => {
                self.with_details(|d| ing(d, id).unit = s.clone());
                if id != 0 {
                    self.edit_ing(
                        id,
                        IngredientEdit {
                            unit: Some(s),
                            ..Default::default()
                        },
                    );
                }
            }
            Message::IngOptional(id, on) => {
                self.with_details(|d| ing(d, id).optional = on);
                if id != 0 {
                    self.edit_ing(
                        id,
                        IngredientEdit {
                            optional: Some(on),
                            ..Default::default()
                        },
                    );
                }
            }
            Message::IngMove(id, by) => match lifecycle::move_ingredient(&mut self.db, id, by) {
                Ok(()) => self.load_parts(),
                Err(e) => self.fail_in_page(e),
            },
            Message::IngDelete(id) => match lifecycle::delete_ingredient(&mut self.db, id) {
                Ok(()) => {
                    self.load_parts();
                    self.reload();
                }
                Err(e) => self.fail_in_page(e),
            },
            // ---- steps ----
            Message::StepAdd => {
                let Some(id) = self.open_id() else {
                    return Task::none();
                };
                match lifecycle::add_step(&mut self.db, id, "") {
                    Ok(_) => {
                        self.load_parts();
                        self.reload();
                    }
                    Err(e) => self.fail_in_page(e),
                }
            }
            Message::StepText(id, action) => {
                let changed = action.is_edit();
                let mut text = None;
                self.with_details(|d| {
                    if let Some(r) = d.steps.iter_mut().find(|r| r.id == id) {
                        r.content.perform(action);
                        if changed {
                            let t = r.content.text().trim_end().to_string();
                            r.doc = organiser_ui::components::rich::render(&t);
                            text = Some(t);
                        }
                    }
                });
                if let Some(t) = text
                    && let Err(e) = lifecycle::edit_step(
                        &mut self.db,
                        id,
                        &StepEdit {
                            text: Some(t),
                            photos: None,
                        },
                    )
                {
                    self.fail_in_page(e);
                }
            }
            Message::StepMove(id, by) => match lifecycle::move_step(&mut self.db, id, by) {
                Ok(()) => self.load_parts(),
                Err(e) => self.fail_in_page(e),
            },
            Message::StepDelete(id) => match lifecycle::delete_step(&mut self.db, id) {
                Ok(()) => {
                    let _ = photos::gc(&self.db, &photos::dir());
                    self.load_parts();
                    self.reload();
                }
                Err(e) => self.fail_in_page(e),
            },
            // ---- photos ----
            Message::PickPhotos(target) => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Add photos")
                            .add_filter("Pictures", &["jpg", "jpeg", "png", "webp", "gif"])
                            .pick_files()
                            .await
                            .map(|v| v.into_iter().map(|f| f.path().to_path_buf()).collect())
                            .unwrap_or_default()
                    },
                    move |v| Message::PhotosPicked(target, v),
                );
            }
            Message::PhotosPicked(target, files) => {
                let mut list = self.photo_list(target);
                for f in files {
                    match photos::import(&photos::dir(), &f) {
                        Ok(name) => {
                            if !list.contains(&name) {
                                list.push(name);
                            }
                        }
                        Err(e) => self.fail_in_page(e),
                    }
                }
                self.save_photos(target, list);
            }
            Message::RemovePhoto(target, name) => {
                let mut list = self.photo_list(target);
                list.retain(|p| *p != name);
                self.save_photos(target, list);
            }
            // ---- cooking ----
            Message::CookedToday => {
                if let Some(id) = self.open_id() {
                    let servings = self.servings_of(id).unwrap_or(1.0);
                    match lifecycle::add_meal(&mut self.db, id, self.today, None, servings, "") {
                        Ok(m) => {
                            self.toast = Some(format!("Logged {} for today", m.title));
                            self.load_shopping();
                            self.reload();
                        }
                        Err(e) => self.fail_in_page(e),
                    }
                }
            }
            Message::PlanIt => {
                if let Some(d) = &self.details {
                    let choice = RecipeChoice {
                        id: d.entry.id,
                        title: d.entry.title.clone(),
                    };
                    return self.more(Message::OpenLog(None, Some(choice)));
                }
            }
            // ---- logging a meal ----
            Message::OpenLog(day, recipe) => {
                let day = day.unwrap_or(self.today);
                self.log_text = day.to_string();
                self.log_view = IadDate::parse(&day.to_string()).unwrap_or_else(IadDate::today);
                if recipe.is_some() {
                    self.cal_recipe = recipe;
                }
                self.log_servings = self
                    .cal_recipe
                    .as_ref()
                    .and_then(|c| self.servings_of(c.id))
                    .map_or("1".to_string(), recipes_schema::units::pretty);
                self.log_time = MealChoice(None);
                self.details = None;
                self.log_open = true;
                self.reload();
            }
            Message::CloseLog => self.log_open = false,
            Message::LogDateText(t) => {
                if let Some(d) = IadDate::parse(&t) {
                    self.log_view = d;
                }
                self.log_text = t;
            }
            Message::LogDatePick(d) => {
                self.log_text = d.format();
                self.log_view = d;
            }
            Message::LogDateView(d) => self.log_view = d,
            Message::LogTime(t) => self.log_time = t,
            Message::LogServings(s) => self.log_servings = s,
            Message::LogSave => {
                let Ok(day) = self.log_text.trim().parse::<chrono::NaiveDate>() else {
                    self.toast = Some("The date should look like 2026-11-30.".into());
                    return Task::none();
                };
                let Some(recipe) = self.cal_recipe.clone() else {
                    self.toast = Some("Pick a recipe first.".into());
                    return Task::none();
                };
                let servings = match parse_qty(&self.log_servings) {
                    Ok(Some(n)) => n,
                    _ => {
                        self.toast = Some("Servings should be a number, at least 1.".into());
                        return Task::none();
                    }
                };
                match lifecycle::add_meal(
                    &mut self.db,
                    recipe.id,
                    day,
                    self.log_time.0,
                    servings,
                    "",
                ) {
                    Ok(m) => {
                        self.log_open = false;
                        self.toast = Some(format!("Logged {}", m.title));
                        self.reload();
                    }
                    Err(e) => self.toast = Some(e.to_string()),
                }
            }
            // ---- calendar ----
            Message::CalRecipe(c) => self.cal_recipe = Some(c),
            Message::MealDelete(id) => match lifecycle::delete_meal(&mut self.db, id) {
                Ok(()) => {
                    self.reload();
                    self.load_shopping();
                }
                Err(e) => self.toast = Some(e.to_string()),
            },
            // ---- fridge ----
            Message::FridgeEdit(on) => self.fridge_edit = on,
            Message::FridgeList(event) => {
                use organiser_ui::iad::primitives::reorder::Event;
                if let Event::Reorder { from, to } = event
                    && let Some(&id) = self.stock_order.items().get(from)
                {
                    self.stock_order.apply(event);
                    let step = to as i32 - from as i32;
                    if let Err(e) = lifecycle::move_stock(&mut self.db, id, step) {
                        self.toast = Some(e.to_string());
                    }
                    self.reload();
                } else {
                    self.stock_order.apply(event);
                }
            }
            Message::StockName(s) => self.stock_new.0 = s,
            Message::StockQty(s) => self.stock_new.1 = s,
            Message::StockUnit(s) => self.stock_new.2 = s,
            Message::StockExpires(s) => self.stock_new.3 = s,
            Message::StockAdd => {
                let (name, qty, unit, exp) = self.stock_new.clone();
                let exp = match parse_date(&exp) {
                    Ok(d) => d,
                    Err(()) => {
                        self.toast = Some("The date should look like 2026-11-30.".into());
                        return Task::none();
                    }
                };
                match parse_qty(&qty) {
                    Ok(q) => match lifecycle::add_stock(&mut self.db, &name, q, &unit, exp) {
                        Ok(_) => {
                            self.stock_new = Default::default();
                            self.reload();
                        }
                        Err(e) => self.toast = Some(e.to_string()),
                    },
                    Err(()) => self.toast = Some("The quantity is not a number.".into()),
                }
            }
            Message::StockRowName(id, s) => {
                if let Some(r) = self.stock.iter_mut().find(|r| r.id == id) {
                    r.name = s.clone();
                }
                // An empty name waits (a name is needed); anything else is saved as typed.
                if !s.trim().is_empty()
                    && let Err(e) = lifecycle::rename_stock(&mut self.db, id, &s)
                {
                    self.toast = Some(e.to_string());
                }
            }
            Message::StockRowQty(id, s) => {
                if let Some(r) = self.stock.iter_mut().find(|r| r.id == id) {
                    r.qty = s.clone();
                }
                match parse_qty(&s) {
                    Ok(q) => {
                        if let Err(e) = lifecycle::edit_stock(&mut self.db, id, Some(q), None, None)
                        {
                            self.toast = Some(e.to_string());
                        }
                    }
                    Err(()) => self.toast = Some("The quantity is not a number.".into()),
                }
            }
            Message::StockRowUnit(id, s) => {
                if let Some(r) = self.stock.iter_mut().find(|r| r.id == id) {
                    r.unit = s.clone();
                }
                if let Err(e) = lifecycle::edit_stock(&mut self.db, id, None, Some(&s), None) {
                    self.toast = Some(e.to_string());
                }
            }
            Message::StockRowExpires(id, s) => {
                if let Some(r) = self.stock.iter_mut().find(|r| r.id == id) {
                    r.expires = s.clone();
                    // Saved once it is a whole date (or empty); half-typed text just waits.
                    if let Ok(d) = parse_date(&s) {
                        r.date = d;
                        if let Err(e) = lifecycle::edit_stock(&mut self.db, id, None, None, Some(d))
                        {
                            self.toast = Some(e.to_string());
                        }
                    }
                }
            }
            Message::StockDelete(id) => match lifecycle::delete_stock(&mut self.db, id) {
                Ok(()) => self.reload(),
                Err(e) => self.toast = Some(e.to_string()),
            },
            // ---- the log ----
            // ---- servings ----
            Message::DServings(s) => {
                let s = s.trim();
                let value = if s.is_empty() {
                    Some(None)
                } else {
                    s.parse::<u32>()
                        .ok()
                        .filter(|n| *n > 0)
                        .map(|n| Some(f64::from(n)))
                };
                if let Some(v) = value {
                    self.edit_servings(v);
                }
            }
            // ---- the shopping list ----
            Message::OpenShop => {
                self.shop_open = true;
                self.reload();
                self.load_shop();
            }
            Message::CloseShop => self.shop_open = false,
            Message::ShopPick(c) => self.shop_pick = Some(c),
            Message::ShopAdd => {
                if let Some(c) = self.shop_pick.take() {
                    if let Some(r) = self.shop.iter_mut().find(|r| r.recipe.id == c.id) {
                        r.portions += 1.0;
                    } else {
                        let servings = self.servings_of(c.id);
                        self.shop.push(ShopRow {
                            recipe: c,
                            portions: servings.unwrap_or(1.0),
                            servings,
                        });
                    }
                    self.rebuild_shop();
                }
            }
            Message::ShopPortions(id, p) => {
                if let Some(r) = self.shop.iter_mut().find(|r| r.recipe.id == id) {
                    r.portions = p.max(1.0);
                }
                self.rebuild_shop();
            }
            Message::ShopRemove(id) => {
                self.shop.retain(|r| r.recipe.id != id);
                self.rebuild_shop();
            }
            Message::ShopPlanned => {
                let from = self.today + Days::new(1);
                let to = chrono::NaiveDate::from_ymd_opt(2100, 12, 31).expect("a date");
                for m in views::meals_between(&self.db, from, to).unwrap_or_default() {
                    let Some(id) = m.recipe_id else { continue };
                    let servings = self.servings_of(id);
                    let portions = m.servings;
                    if let Some(r) = self.shop.iter_mut().find(|r| r.recipe.id == id) {
                        r.portions += portions;
                    } else {
                        self.shop.push(ShopRow {
                            recipe: RecipeChoice { id, title: m.title },
                            portions,
                            servings,
                        });
                    }
                }
                self.rebuild_shop();
            }
            Message::ShopClear => {
                self.shop.clear();
                self.rebuild_shop();
            }
            Message::ShopCopy => {
                let md = views::shopping_markdown(&self.shop_items);
                self.toast = Some("Copied: paste it into a Todo note".into());
                return iced::clipboard::write(md);
            }
            Message::ShopSave => {
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Save the shopping list")
                            .set_file_name("shopping-list.md")
                            .save_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    Message::ShopSaved,
                );
            }
            Message::ShopSaved(Some(p)) => {
                let md = views::shopping_markdown(&self.shop_items) + "\n";
                self.toast = Some(match std::fs::write(&p, md) {
                    Ok(()) => format!("Saved {}", p.display()),
                    Err(e) => e.to_string(),
                });
            }
            _ => {}
        }
        Task::none()
    }

    fn edit_servings(&mut self, v: Option<f64>) {
        self.edit(RecipeEdit {
            servings: Some(v),
            ..RecipeEdit::default()
        });
    }

    fn edit_ing(&mut self, id: i64, edit: IngredientEdit) {
        if let Err(e) = lifecycle::edit_ingredient(&mut self.db, id, &edit) {
            self.fail_in_page(e);
        } else {
            self.with_details(|_| {});
            self.load_shopping();
        }
    }
}

/// The ingredient row with this id; 0 is the one being typed at the end.
fn ing(d: &mut crate::app::Details, id: i64) -> &mut crate::app::IngRow {
    match d.ings.iter().position(|r| r.id == id) {
        Some(at) if id != 0 => &mut d.ings[at],
        _ => &mut d.new_ing,
    }
}
