//  This file is part of Sulis, a turn based RPG written in Rust.
//  Copyright 2018 Jared Stephen
//
//  Sulis is free software: you can redistribute it and/or modify
//  it under the terms of the GNU General Public License as published by
//  the Free Software Foundation, either version 3 of the License, or
//  (at your option) any later version.
//
//  Sulis is distributed in the hope that it will be useful,
//  but WITHOUT ANY WARRANTY; without even the implied warranty of
//  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//  GNU General Public License for more details.
//
//  You should have received a copy of the GNU General Public License
//  along with Sulis.  If not, see <http://www.gnu.org/licenses/>

use std::any::Any;
use std::rc::Rc;
use std::cell::RefCell;

use sulis_core::ui::{Widget, WidgetKind, WidgetState};
use sulis_widgets::{TextArea};
use sulis_module::{Ability, Module};

use inventory_window::add_bonus_text_args;

pub const NAME: &str = "ability_pane";

pub struct AbilityPane {
    ability: Option<Rc<Ability>>,
    pub details: Rc<RefCell<Widget>>,
}

impl AbilityPane {
    pub fn empty() -> Rc<RefCell<AbilityPane>> {
        Rc::new(RefCell::new(AbilityPane {
            ability: None,
            details: Widget::with_theme(TextArea::empty(), "details"),
        }))
    }

    pub fn new(ability: Rc<Ability>) -> Rc<RefCell<AbilityPane>> {
        Rc::new(RefCell::new(AbilityPane {
            ability: Some(ability),
            details: Widget::with_theme(TextArea::empty(), "details"),
        }))
    }

    pub fn clear_ability(&mut self) {
        self.ability = None;
    }

    pub fn set_ability(&mut self, ability: Rc<Ability>) {
        self.ability = Some(ability);
    }
}

impl WidgetKind for AbilityPane {
    fn get_name(&self) -> &str { NAME }
    fn as_any(&self) -> &Any { self }
    fn as_any_mut(&mut self) -> &mut Any { self }

    fn on_add(&mut self, _widget: &Rc<RefCell<Widget>>) -> Vec<Rc<RefCell<Widget>>> {
        let ability = match self.ability {
            None => return Vec::new(),
            Some(ref ability) => ability,
        };

        add_ability_text_args(&mut self.details.borrow_mut().state, &ability);

        vec![Rc::clone(&self.details)]
    }
}

pub fn add_ability_text_args(state: &mut WidgetState, ability: &Rc<Ability>) {
    state.add_text_arg("name", &ability.name);
    state.add_text_arg("description", &ability.description);

    if let Some(ref active) = ability.active {
        state.add_text_arg("active", "true");
        let ap = active.ap / Module::rules().display_ap;
        state.add_text_arg("activate_ap", &ap.to_string());
    } else {
        state.add_text_arg("passive", "true");
    }

    add_bonus_text_args(&ability.bonuses, state);

    if let Some(ref prereqs) = ability.prereqs {
        state.add_text_arg("prereqs", "true");

        if let Some(ref attrs) = prereqs.attributes {
            for &(attr, amount) in attrs.iter() {
                state.add_text_arg(&format!("prereq_{}", attr.short_name()), &amount.to_string());
            }
        }

        for (index, &(ref class, level)) in prereqs.levels.iter().enumerate() {
            state.add_text_arg(&format!("prereq_class_{}", index), &class.id);
            state.add_text_arg(&format!("prereq_level_{}", index), &level.to_string());
        }

        if let Some(total_level) = prereqs.total_level {
            state.add_text_arg("prereq_total_level", &total_level.to_string());
        }

        if let Some(ref race) = prereqs.race {
            state.add_text_arg("prereq_race", &race.id);
        }

        for (index, ref ability_id) in prereqs.abilities.iter().enumerate() {
            let ability = match Module::ability(ability_id) {
                None => {
                    warn!("No ability '{}' found for prereq list", ability_id);
                    continue;
                }, Some(ability) => ability,
            };

            state.add_text_arg(&format!("prereq_ability_{}", index), &ability.name);
        }
    }
}