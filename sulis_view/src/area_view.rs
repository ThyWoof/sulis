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
use std::cell::{RefCell, RefMut};
use std::cmp;
use std::time;

use sulis_core::ui::{compute_area_scaling, animation_state};
use sulis_core::ui::{color, Cursor, WidgetKind, Widget};
use sulis_core::io::*;
use sulis_core::io::event::ClickKind;
use sulis_core::util::{self, Point};
use sulis_core::config::CONFIG;
use sulis_core::resource::Sprite;
use sulis_core::extern_image::ImageBuffer;
use sulis_module::area::Layer;
use sulis_state::{AreaState, EntityState, GameState};

use {ActionMenu, EntityMouseover, PropMouseover};

struct HoverSprite {
    pub sprite: Rc<Sprite>,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub left_click_action_valid: bool,
}

const NAME: &'static str = "area";

pub struct AreaView {
    mouse_over: Rc<RefCell<Widget>>,
    scale: (f32, f32),
    cache_invalid: bool,
    layers: Vec<String>,

    scroll_x: f32,
    scroll_y: f32,
    max_scroll_x: f32,
    max_scroll_y: f32,

    hover_sprite: Option<HoverSprite>,
}

const TILE_CACHE_TEXTURE_SIZE: u32 = 2048;
const TILE_SIZE: u32 = 16;
const TEX_COORDS: [f32; 8] = [ 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0 ];

const VISIBILITY_TEX_ID: &'static str = "__visibility__";

impl AreaView {
    pub fn new(mouse_over: Rc<RefCell<Widget>>) -> Rc<RefCell<AreaView>> {
        Rc::new(RefCell::new(AreaView {
            mouse_over: mouse_over,
            scale: (1.0, 1.0),
            hover_sprite: None,
            cache_invalid: true,
            layers: Vec::new(),
            scroll_x: 0.0,
            scroll_y: 0.0,
            max_scroll_x: 0.0,
            max_scroll_y: 0.0,
        }))
    }

    pub fn center_scroll_on(&mut self, entity: &Rc<RefCell<EntityState>>, area_width: i32,
                            area_height: i32, inner_width: i32, inner_height: i32) {
        self.recompute_max_scroll(area_width, area_height, inner_width, inner_height);
        let (scale_x, scale_y) = self.scale;

        let x = entity.borrow().location.x as f32 + entity.borrow().size.size as f32 / 2.0;
        let y = entity.borrow().location.y as f32 + entity.borrow().size.size as f32 / 2.0;
        let x = x - inner_width as f32 / scale_x / 2.0;
        let y = y - inner_height as f32 / scale_y / 2.0;

        self.set_scroll(x, y);
    }

    pub fn set_scroll(&mut self, mut scroll_x: f32, mut scroll_y: f32) {
        if scroll_x < 0.0 { scroll_x = 0.0; }
        else if scroll_x > self.max_scroll_x { scroll_x = self.max_scroll_x; }

        if scroll_y < 0.0 { scroll_y = 0.0; }
        else if scroll_y > self.max_scroll_y { scroll_y = self.max_scroll_y; }

        self.scroll_x = scroll_x;
        self.scroll_y = scroll_y;
    }

    fn recompute_max_scroll(&mut self, area_width: i32, area_height: i32,
                            inner_width: i32, inner_height: i32) {
        let (scale_x, scale_y) = self.scale;
        self.max_scroll_x = area_width as f32 - inner_width as f32 / scale_x;
        self.max_scroll_y = area_height as f32 - inner_height as f32 / scale_y;
        if self.max_scroll_x < 0.0 { self.max_scroll_x = 0.0; }
        if self.max_scroll_y < 0.0 { self.max_scroll_y = 0.0; }
    }

    pub fn get_mouseover_pos(&self, x: i32, y: i32, width: i32, height: i32) -> (i32, i32) {
        let x = x as f32 + width as f32 / 2.0;
        let y = y as f32 + height as f32;
        let x = ((x - self.scroll_x) * self.scale.0).round() as i32;
        let y = ((y - self.scroll_y) * self.scale.1).round() as i32;

        (x, y)
    }

    fn get_cursor_pos(&self, widget: &Rc<RefCell<Widget>>) -> (i32, i32) {
        let pos = widget.borrow().state.inner_position;
        let (x, y) = self.get_cursor_pos_scaled(pos.x, pos.y);
        ((x + self.scroll_x) as i32, (y + self.scroll_y) as i32)
    }

    fn get_cursor_pos_scaled(&self, pos_x: i32, pos_y: i32) -> (f32, f32) {
        let mut x = Cursor::get_x_f32() - pos_x as f32;
        let mut y = Cursor::get_y_f32() - pos_y as f32;

        let (scale_x, scale_y) = self.scale;
        x = x / scale_x;
        y = y / scale_y;

        (x, y)
    }

    fn draw_layer_to_texture(&self, renderer: &mut GraphicsRenderer, layer: &Layer, texture_id: &str) {
        let (max_tile_x, max_tile_y) = AreaView::get_texture_cache_max(layer.width, layer.height);
        let mut draw_list = DrawList::empty_sprite();

        for tile_y in 0..max_tile_y {
            for tile_x in 0..max_tile_x {
                let tile = match layer.tile_at(tile_x, tile_y) {
                    &None => continue,
                    &Some(ref tile) => tile,
                };
                let sprite = &tile.image_display;

                draw_list.append(&mut DrawList::from_sprite(sprite, tile_x, tile_y,
                                                            tile.width, tile.height));
            }
        }

        AreaView::draw_list_to_texture(renderer, draw_list, texture_id);
    }

    fn draw_visibility_to_texture(&self, renderer: &mut GraphicsRenderer, sprite: &Rc<Sprite>,
                                  area_state: &RefMut<AreaState>) {
        let start_time = time::Instant::now();
        renderer.clear_texture(VISIBILITY_TEX_ID);
        let (max_tile_x, max_tile_y) = AreaView::get_texture_cache_max(area_state.area.width,
                                                                       area_state.area.height);

        let mut draw_list = DrawList::empty_sprite();

        for tile_y in 0..max_tile_y {
            for tile_x in 0..max_tile_x {
                if area_state.is_pc_visible(tile_x, tile_y) { continue; }
                draw_list.append(&mut DrawList::from_sprite(sprite, tile_x, tile_y, 1, 1));
            }
        }

        AreaView::draw_list_to_texture(renderer, draw_list, VISIBILITY_TEX_ID);
        trace!("Visibility render to texture time: {}",
              util::format_elapsed_secs(start_time.elapsed()));
    }

    fn draw_list_to_texture(renderer: &mut GraphicsRenderer, draw_list: DrawList, texture_id: &str) {
        let mut draw_list = draw_list;
        draw_list.texture_mag_filter = TextureMagFilter::Linear;
        draw_list.texture_min_filter = TextureMinFilter::Linear;
        draw_list.set_scale(TILE_SIZE as f32 / TILE_CACHE_TEXTURE_SIZE as f32 *
                            CONFIG.display.width as f32,
                            TILE_SIZE as f32 / TILE_CACHE_TEXTURE_SIZE as f32 *
                            CONFIG.display.height as f32);
        renderer.draw_to_texture(texture_id, draw_list);
    }

    fn get_texture_cache_max(width: i32, height: i32) -> (i32, i32) {
        let x = cmp::min((TILE_CACHE_TEXTURE_SIZE / TILE_SIZE) as i32, width);
        let y = cmp::min((TILE_CACHE_TEXTURE_SIZE / TILE_SIZE) as i32, height);

        (x, y)
    }

    fn draw_layer(&self, renderer: &mut GraphicsRenderer, scale_x: f32, scale_y: f32,
                  widget: &Widget, id: &str) {
        let p = widget.state.inner_position;
        let mut draw_list =
            DrawList::from_texture_id(&id, &TEX_COORDS,
                                      p.x as f32 - self.scroll_x,
                                      p.y as f32 - self.scroll_y,
                                      (TILE_CACHE_TEXTURE_SIZE / TILE_SIZE) as f32,
                                      (TILE_CACHE_TEXTURE_SIZE / TILE_SIZE) as f32);
        draw_list.set_scale(scale_x, scale_y);
        renderer.draw(draw_list);
    }

    fn draw_entities(&self, renderer: &mut GraphicsRenderer, scale_x: f32, scale_y: f32,
                     _alpha: f32, widget: &Widget, state: &AreaState, millis: u32) {
        let p = widget.state.inner_position;

        let mut draw_list = DrawList::empty_sprite();
        draw_list.set_scale(scale_x, scale_y);
        for prop_state in state.prop_iter() {
            let x = (prop_state.location.x + p.x) as f32 - self.scroll_x;
            let y = (prop_state.location.y + p.y) as f32 - self.scroll_y;
            prop_state.append_to_draw_list(&mut draw_list, x, y, millis);
        }

        if !draw_list.is_empty() {
            renderer.draw(draw_list);
        }

        for entity in state.entity_iter() {
            let entity = entity.borrow();

            if entity.location_points().any(|p| state.is_pc_visible(p.x, p.y)) {
                let x = (entity.location.x + p.x) as f32 - self.scroll_x + entity.sub_pos.0;
                let y = (entity.location.y + p.y) as f32 - self.scroll_y + entity.sub_pos.1;

                // TODO implement drawing with alpha
                entity.actor.draw_graphics_mode(renderer, scale_x, scale_y,
                                                x, y, millis);
            }
        }
    }
}

const BASE_LAYER_ID: &str = "base_layer";
const AERIAL_LAYER_ID: &str = "aerial_layer";

impl WidgetKind for AreaView {
    fn get_name(&self) -> &str { NAME }

    fn as_any(&self) -> &Any { self }

    fn as_any_mut(&mut self) -> &mut Any { self }

    fn on_add(&mut self, _widget: &Rc<RefCell<Widget>>) -> Vec<Rc<RefCell<Widget>>> {
        self.hover_sprite = None;
        self.mouse_over.borrow_mut().state.add_text_arg("0", "");
        self.mouse_over.borrow_mut().state.add_text_arg("1", "");

        let area_state = GameState::area_state();
        let area = &area_state.borrow().area;

        for layer in area.terrain.layers.iter() {
            self.layers.push(layer.id.to_string());
        }
        self.cache_invalid = true;

        Vec::with_capacity(0)
    }

    fn draw_graphics_mode(&mut self, renderer: &mut GraphicsRenderer, pixel_size: Point,
                          widget: &Widget, millis: u32) {
        self.scale = compute_area_scaling(pixel_size);
        let (scale_x, scale_y) = self.scale;

        let area_state = GameState::area_state();
        let mut state = area_state.borrow_mut();

        match state.pop_scroll_to_callback() {
            None => (),
            Some(entity) => self.center_scroll_on(&entity, state.area.width, state.area.height,
                                                  widget.state.inner_width(), widget.state.inner_height()),
        }

        if self.cache_invalid {
            debug!("Caching area '{}' layers to texture", state.area.id);

            let texture_ids = vec![VISIBILITY_TEX_ID, BASE_LAYER_ID, AERIAL_LAYER_ID];
            for texture_id in texture_ids {
                if renderer.has_texture(texture_id) {
                    renderer.clear_texture(texture_id);
                } else {
                    renderer.register_texture(texture_id,
                                              ImageBuffer::new(TILE_CACHE_TEXTURE_SIZE, TILE_CACHE_TEXTURE_SIZE),
                                              TextureMinFilter::Nearest,
                                              TextureMagFilter::Nearest);
                }
            }

            for (index, layer) in state.area.terrain.layers.iter().enumerate() {
                let texture_id = if index <= state.area.terrain.entity_layer_index {
                    BASE_LAYER_ID
                } else {
                    AERIAL_LAYER_ID
                };
                trace!("Caching layer '{}'", layer.id);

                self.draw_layer_to_texture(renderer, &layer, texture_id);
            }

            self.cache_invalid = false;
        }

        if state.pc_vis_cache_invalid {
            trace!("Redrawing PC visibility to texture");
            self.draw_visibility_to_texture(renderer, &state.area.visibility_tile, &state);
            state.pc_vis_cache_invalid = false;
        }

        let p = widget.state.inner_position;

        self.draw_layer(renderer, scale_x, scale_y, widget, BASE_LAYER_ID);

        let mut draw_list = DrawList::empty_sprite();
        for transition in state.area.transitions.iter() {
            draw_list.set_scale(scale_x, scale_y);
            transition.image_display.append_to_draw_list(&mut draw_list, &animation_state::NORMAL,
                                                        (transition.from.x + p.x) as f32 - self.scroll_x,
                                                        (transition.from.y + p.y) as f32 - self.scroll_y,
                                                        transition.size.width as f32,
                                                        transition.size.height as f32,
                                                        millis);
        }

        if !draw_list.is_empty() {
            renderer.draw(draw_list);
        }

        self.draw_entities(renderer, scale_x, scale_y, 1.0, widget, &state, millis);

        self.draw_layer(renderer, scale_x, scale_y, widget, AERIAL_LAYER_ID);

        self.draw_layer(renderer, scale_x, scale_y, widget, VISIBILITY_TEX_ID);

        //draw transparent version of each actor for when they are obscured behind objects
        // self.draw_entities(renderer, scale_x, scale_y, 0.4, widget, &state, millis);

        if let Some(ref hover) = self.hover_sprite {
            let mut draw_list = DrawList::from_sprite_f32(&hover.sprite,
                                                          (hover.x + p.x) as f32 - self.scroll_x,
                                                          (hover.y + p.y) as f32 - self.scroll_y,
                                                          hover.w as f32, hover.h as f32);
            if !hover.left_click_action_valid { draw_list.set_color(color::RED); }
            draw_list.set_scale(scale_x, scale_y);
            renderer.draw(draw_list);
        }

        for feedback_text in state.feedback_text_iter() {
            feedback_text.draw(renderer, p.x as f32 - self.scroll_x, p.y as f32- self.scroll_y,
                               scale_x, scale_y);
        }


        GameState::draw_graphics_mode(renderer, p.x as f32 - self.scroll_x, p.y as f32- self.scroll_y,
                                      scale_x, scale_y, millis);
    }

    fn on_mouse_release(&mut self, widget: &Rc<RefCell<Widget>>, kind: ClickKind) -> bool {
        self.super_on_mouse_release(widget, kind);
        let (x, y) = self.get_cursor_pos(widget);
        if x < 0 || y < 0 { return true; }

        let area_state = GameState::area_state();
        let action_menu = ActionMenu::new(x, y);
        if kind == ClickKind::Left {
            action_menu.borrow().fire_default_callback(&widget, self);

            if let Some(entity) = area_state.borrow().get_entity_at(x, y) {
                let (x, y) = {
                    let entity = entity.borrow();
                    self.get_mouseover_pos(entity.location.x, entity.location.y,
                        entity.size.size, entity.size.size)
                };
                Widget::set_mouse_over(widget, EntityMouseover::new(&entity), x, y);
            }

        } else if kind == ClickKind::Right {
            Widget::add_child_to(widget, Widget::with_defaults(action_menu));
        }

        true
    }

    fn on_mouse_drag(&mut self, widget: &Rc<RefCell<Widget>>, kind: ClickKind,
                     delta_x: f32, delta_y: f32) -> bool {
        let area_state = GameState::area_state();
        let area_width = area_state.borrow().area.width;
        let area_height = area_state.borrow().area.height;
        self.recompute_max_scroll(area_width, area_height, widget.borrow().state.inner_width(),
            widget.borrow().state.inner_height());

        match kind {
            ClickKind::Middle => {
                let x = self.scroll_x - delta_x;
                let y = self.scroll_y - delta_y;
                self.set_scroll(x, y);
            },
            _ => (),
        }

        true
    }

    fn on_mouse_move(&mut self, widget: &Rc<RefCell<Widget>>,
                     _delta_x: f32, _delta_y: f32) -> bool {
        let (area_x, area_y) = self.get_cursor_pos(widget);
        let area_state = GameState::area_state();

        {
            let ref mut state = self.mouse_over.borrow_mut().state;
            state.clear_text_args();
            state.add_text_arg("0", &format!("{}", area_x));
            state.add_text_arg("1", &format!("{}", area_y));
        }
        self.mouse_over.borrow_mut().invalidate_layout();
        self.hover_sprite = None;

        if let Some(entity) = area_state.borrow().get_entity_at(area_x, area_y) {
            let (x, y) = {
                let entity = entity.borrow();
                self.get_mouseover_pos(entity.location.x, entity.location.y,
                    entity.size.size, entity.size.size)
            };
            Widget::set_mouse_over(widget, EntityMouseover::new(&entity), x, y);

            // let pc = GameState::pc();
            // if *pc.borrow() != *entity.borrow() {
            //     let sprite = &entity.borrow().size.cursor_sprite;
            //     let x = entity.borrow().location.x as f32 - self.scroll_x;
            //     let y = entity.borrow().location.y as f32 - self.scroll_y;
            //     let size = entity.borrow().size() as f32;
            //
            //     let mut cursor = DrawList::from_sprite_f32(sprite, x, y, size, size);
            //     cursor.set_color(color::RED);
            //     self.add_cursor(cursor);
            // }
        } else if let Some(index) = area_state.borrow().prop_index_at(area_x, area_y) {
            let (x, y) = {
                let prop_state = &area_state.borrow().props[index];
                self.get_mouseover_pos(prop_state.location.x, prop_state.location.y,
                                       prop_state.prop.width as i32, prop_state.prop.height as i32)
            };
            Widget::set_mouse_over(widget, PropMouseover::new(index), x, y);
        }

        let pc = GameState::pc();
        let size = &pc.borrow().size;
        let action_menu = ActionMenu::new(area_x, area_y);
        let left_click_action_valid = action_menu.borrow().is_default_callback_valid();

        let hover_sprite = HoverSprite {
            sprite: Rc::clone(&size.cursor_sprite),
            x: area_x - size.size / 2,
            y: area_y - size.size / 2,
            w: size.size,
            h: size.size,
            left_click_action_valid,
        };
        self.hover_sprite = Some(hover_sprite);

        true
    }

    fn on_mouse_exit(&mut self, widget: &Rc<RefCell<Widget>>) -> bool {
        self.super_on_mouse_exit(widget);
        self.mouse_over.borrow_mut().state.clear_text_args();
        self.hover_sprite = None;
        true
    }
}
