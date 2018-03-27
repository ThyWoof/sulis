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

use sulis_core::io::{InputAction, MainLoopUpdater};
use sulis_core::ui::*;
use sulis_state::ActorState;
use sulis_module::{Actor, Module};
use sulis_widgets::{Button, ConfirmationWindow, Label, TextArea};

use character_window::create_details_text_box;
use {CharacterBuilder, LoadingScreen};

pub struct LoopUpdater {
    view: Rc<RefCell<CharacterSelector>>,
}

impl LoopUpdater {
    pub fn new(view: &Rc<RefCell<CharacterSelector>>) -> LoopUpdater {
        LoopUpdater {
            view: Rc::clone(view),
        }
    }
}

impl MainLoopUpdater for LoopUpdater {
    fn update(&self, _root: &Rc<RefCell<Widget>>, _millis: u32) { }

    fn is_exit(&self) -> bool {
        self.view.borrow().is_exit()
    }
}

pub struct CharacterSelector {
    selected: Option<Rc<Actor>>,

    to_select: Option<String>,
}

impl CharacterSelector {
    pub fn new() -> Rc<RefCell<CharacterSelector>> {
        Rc::new(RefCell::new(CharacterSelector {
            selected: None,
            to_select: None,
        }))
    }

    pub fn is_exit(&self) -> bool {
        EXIT.with(|exit| *exit.borrow())
    }

    pub fn selected(&self) -> Option<Rc<Actor>> {
        self.selected.clone()
    }

    pub fn set_to_select(&mut self, id: &str) {
        self.to_select = Some(id.to_string());
    }
}

thread_local! {
    static EXIT: RefCell<bool> = RefCell::new(false);
}

impl WidgetKind for CharacterSelector {
    fn get_name(&self) -> &str { "root" }
    fn as_any(&self) -> &Any { self }
    fn as_any_mut(&mut self) -> &mut Any { self }

    fn on_key_press(&mut self, widget: &Rc<RefCell<Widget>>, key: InputAction) -> bool {
        use sulis_core::io::InputAction::*;
        match key {
            ShowMenu => {
                let exit_window = Widget::with_theme(
                    ConfirmationWindow::new(Callback::new(Rc::new(|widget, _| {
                        let parent = Widget::get_root(&widget);
                        let selector = Widget::downcast_kind_mut::<CharacterSelector>(&parent);
                        selector.selected = None;
                        EXIT.with(|exit| *exit.borrow_mut() = true);
                    }))),
                    "exit_confirmation_window");
                exit_window.borrow_mut().state.set_modal(true);
                Widget::add_child_to(&widget, exit_window);
            },
            _ => return false,
        }

        true
    }

    fn on_add(&mut self, _widget: &Rc<RefCell<Widget>>) -> Vec<Rc<RefCell<Widget>>> {
        debug!("Adding to main menu widget");

        let title = Widget::with_theme(Label::empty(), "title");
        let chars_title = Widget::with_theme(Label::empty(), "characters_title");

        let new_character_button = Widget::with_theme(Button::empty(), "new_character_button");
        new_character_button.borrow_mut().state.add_callback(Callback::new(Rc::new(|widget, _| {
            let parent = Widget::get_parent(&widget);

            let builder = Widget::with_defaults(CharacterBuilder::new());
            builder.borrow_mut().state.set_modal(true);
            Widget::add_child_to(&parent, builder);
        })));

        let delete_character_button = Widget::with_theme(Button::empty(), "delete_character_button");

        let (actor_name, actor_id) = match self.selected {
            None => (String::new(), String::new()),
            Some(ref actor) => (actor.name.to_string(), actor.id.to_string()),
        };
        delete_character_button.borrow_mut().state.add_callback(Callback::new(Rc::new(move |widget, _| {
            let root = Widget::get_root(&widget);

            let actor_id = actor_id.clone();
            let window = ConfirmationWindow::new(Callback::new(Rc::new(move |widget, _| {
                Module::delete_character(&actor_id);
                widget.borrow_mut().mark_for_removal();

                let root = Widget::get_root(&widget);
                let selector = Widget::downcast_kind_mut::<CharacterSelector>(&root);
                selector.selected = None;
                root.borrow_mut().invalidate_children();
            })));
            {
                let window = window.borrow();
                window.title().borrow_mut().state.add_text_arg("name", &actor_name);
            }
            let window_widget = Widget::with_theme(window, "delete_character_confirmation_window");
            window_widget.borrow_mut().state.set_modal(true);
            Widget::add_child_to(&root, window_widget);
        })));
        delete_character_button.borrow_mut().state.set_enabled(self.selected.is_some());

        let characters_pane = Widget::empty("characters_pane");
        {
            let characters = Module::get_available_characters();
            for actor in characters {
                let actor = Rc::new(actor);
                trace!("Adding button for {}", actor.id);

                let select = if let Some(ref to_select_id) = self.to_select {
                    if &actor.id == to_select_id {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };

                if select {
                    self.selected = Some(Rc::clone(&actor));
                    self.to_select = None;
                }

                let actor_button = Widget::with_theme(Button::empty(), "character_button");
                actor_button.borrow_mut().state.add_text_arg("name", &actor.name);
                if let Some(ref portrait) = actor.portrait {
                    actor_button.borrow_mut().state.add_text_arg("portrait", &portrait.id());
                }

                if let Some(ref selected) = self.selected {
                    if actor.id == selected.id {
                        actor_button.borrow_mut().state.set_active(true);
                    }
                }

                actor_button.borrow_mut().state.add_callback(actor_callback(actor));

                Widget::add_child_to(&characters_pane, actor_button);
            }
        }

        let play_button = Widget::with_theme(Button::empty(), "play_button");
        play_button.borrow_mut().state.set_enabled(self.selected.is_some());
        play_button.borrow_mut().state.add_callback(Callback::new(Rc::new(|widget, _| {
            EXIT.with(|exit| *exit.borrow_mut() = true);

            let root = Widget::get_root(&widget);
            let loading_screen = Widget::with_defaults(LoadingScreen::new());
            loading_screen.borrow_mut().state.set_modal(true);
            Widget::add_child_to(&root, loading_screen);
        })));

        let details = if let Some(ref actor) = self.selected {
            let mut actor_state = ActorState::new(Rc::clone(actor));
            actor_state.compute_stats();
            actor_state.init();
            actor_state.init_turn();
            create_details_text_box(&actor_state)
        } else {
            Widget::with_theme(TextArea::empty(), "details")
        };

        vec![title, chars_title, characters_pane, new_character_button,
            delete_character_button, play_button, details]
    }
}

fn actor_callback(actor: Rc<Actor>) -> Callback {
    Callback::new(Rc::new(move |widget, _| {
        let parent = Widget::go_up_tree(&widget, 2);
        let selector = Widget::downcast_kind_mut::<CharacterSelector>(&parent);
        selector.selected = Some(Rc::clone(&actor));
        parent.borrow_mut().invalidate_children();
    }))
}