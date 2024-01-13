use egui::{Align2, Area, Color32, Key, Painter, Response, RichText, Ui, Window};
use std::cell::RefCell;
use std::rc::Rc;
use walkers::{Plugin, Position, Projector};

#[derive(Clone)]
struct DrawedLine {
    color: egui::Color32,
    points: Vec<Position>,
}

impl Default for DrawedLine {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            color: egui::Color32::RED,
        }
    }
}

impl DrawedLine {
    fn clear(&mut self) {
        self.points.clear()
    }
}

struct MapPainter {
    /// Current line.
    /// Move points to completed at first 'drag_released' event
    current: DrawedLine,
    /// Completed lines
    completed: Vec<DrawedLine>,
    /// Forward history
    forward_history: Vec<DrawedLine>,
    painting_mode_enabled: bool,
    ignore_painting: bool,
}

impl MapPainter {
    fn new() -> Self {
        Self {
            current: Default::default(),
            completed: Vec::new(),
            forward_history: Vec::new(),
            painting_mode_enabled: false,
            ignore_painting: false,
        }
    }
}

impl MapPainter {
    fn set_color(&mut self, color: egui::Color32) {
        self.current.color = color;
    }

    fn handle_paint(&mut self, response: &Response, projector: &Projector) {
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(offset) = response
                .hover_pos()
                .map(|x| projector.reverse(x - response.rect.center()))
            {
                self.current.points.push(offset);
            }
        }

        if response.drag_released_by(egui::PointerButton::Primary) {
            self.completed.push(self.current.clone());
            self.current.clear();
            self.forward_history.clear();
        }
    }

    fn discard_last_paint(&mut self) {
        self.current.clear();
    }

    fn draw_lines(&self, painter: Painter, projector: &Projector) {
        for line in [self.completed.iter(), [self.current.clone()].iter()]
            .into_iter()
            .flatten()
        {
            let mut points = line.points.iter();
            if let Some(first) = points.next() {
                let mut prev_point = first;
                for point in points {
                    painter.line_segment(
                        [
                            projector.project(*prev_point).to_pos2(),
                            projector.project(*point).to_pos2(),
                        ],
                        (2.5, line.color),
                    );
                    prev_point = point;
                }
            }
        }
    }

    /// Undo last drawed line
    fn undo_line(&mut self) {
        if let Some(last_line) = self.completed.pop() {
            self.forward_history.push(last_line);
        }
    }

    fn redo_line(&mut self) {
        if let Some(next_line) = self.forward_history.pop() {
            self.completed.push(next_line);
        }
    }
}

pub struct MapPainterPlugin {
    painter: Rc<RefCell<MapPainter>>,
    active_color: egui::Color32,
    show_palette: bool,
}

impl Default for MapPainterPlugin {
    fn default() -> Self {
        Self::new()
    }
}

const BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(28.0, 28.0);
const SPACER_SIZE: f32 = 16.0;

impl MapPainterPlugin {
    pub fn new() -> Self {
        Self {
            painter: Rc::new(RefCell::new(MapPainter::new())),
            active_color: egui::Color32::RED,
            show_palette: false,
        }
    }

    fn palette_ui(&mut self, ui: &mut Ui, colors_and_keys: Vec<(Color32, Key)>) {
        ui.horizontal(|ui| {
            for (color, key) in colors_and_keys.iter() {
                let color_button = egui::Button::new(key.name().to_string()).fill(*color);

                if ui
                    .add_sized(BUTTON_SIZE, color_button)
                    .on_hover_text(format!("Shortcut: {}", key.name()))
                    .clicked()
                {
                    self.active_color = *color;
                    self.painter.borrow_mut().set_color(*color);
                    self.show_palette = false;
                }
            }
        });
    }

    fn show_ui_palette(&mut self, ui: &Ui) {
        let colors_and_keys = [
            (egui::Color32::RED, egui::Key::Num1),
            (egui::Color32::BLUE, egui::Key::Num2),
            (egui::Color32::GREEN, egui::Key::Num3),
            (egui::Color32::BROWN, egui::Key::Num4),
        ];

        if self.show_palette {
            Window::new("Palette")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::LEFT_TOP, [54.0, 60.0])
                .show(ui.ctx(), |ui| {
                    self.palette_ui(ui, colors_and_keys.to_vec());

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        self.show_palette = false;
                    }
                });
        } else {
            /* choose color without palette */
            for (color, key) in colors_and_keys.iter() {
                if ui.input(|i| i.key_pressed(*key)) {
                    self.active_color = *color;
                    self.painter.borrow_mut().set_color(*color);
                    self.show_palette = false;
                }
            }
        }
    }

    fn show_ui_edit(&mut self, ui: &Ui) {
        let (painting_mode, has_lines, has_forward_history) = {
            let painter = self.painter.borrow();

            (
                painter.painting_mode_enabled,
                !painter.completed.is_empty(),
                !painter.forward_history.is_empty(),
            )
        };

        Area::new("Edits")
            .anchor(Align2::LEFT_TOP, [16., 104.])
            .show(ui.ctx(), |ui| {
                if has_lines {
                    if ui
                        .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("S").heading()))
                        .on_hover_text("Send figure\nShortcut: SHIFT+S")
                        .clicked()
                        || ui.input_mut(|i| {
                            i.consume_shortcut(&egui::KeyboardShortcut {
                                modifiers: egui::Modifiers::SHIFT,
                                logical_key: egui::Key::S,
                            })
                        })
                    {
                        log::error!("Not implemented: Send figure");
                    }
                } else {
                    ui.add_space(BUTTON_SIZE.x);
                }
                if painting_mode {
                    ui.add_space(8.0);
                    if has_forward_history {
                        if ui
                            .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("R").heading()))
                            .on_hover_text("Redo\nShortcut: R")
                            .clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::R))
                        {
                            self.painter.borrow_mut().redo_line();
                        }
                    } else {
                        ui.add_space(BUTTON_SIZE.x);
                    }
                    ui.add_space(8.0);
                    if has_lines {
                        if ui
                            .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("U").heading()))
                            .on_hover_text("Undo\nShortcut: U")
                            .clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::U))
                        {
                            self.painter.borrow_mut().undo_line();
                        }
                    } else {
                        ui.add_space(BUTTON_SIZE.x);
                    }
                }
            });
    }

    fn ui_painting(&mut self, ui: &mut Ui) {
        if ui
            .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("🗙").heading()))
            .on_hover_text("Cancel painting\nShortcut: D or Escape")
            .clicked()
            || ui.input(|i| i.key_pressed(egui::Key::D))
        {
            self.painter.borrow_mut().painting_mode_enabled = false;
            self.show_palette = false;
        }

        let color_button = egui::Button::new("").fill(self.active_color);

        ui.add_space(SPACER_SIZE);

        if ui
            .add_sized(BUTTON_SIZE, color_button)
            .on_hover_text("Choose color\nShortcut: C")
            .clicked()
            || ui.input(|i| i.key_pressed(egui::Key::C))
        {
            self.show_palette = !self.show_palette;
        }
    }

    fn ui_short(&mut self, ui: &mut Ui) {
        if ui
            .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("📓").heading()))
            .on_hover_text("Painting mode\nShortcut: D")
            .clicked()
            || ui.input(|i| i.key_pressed(egui::Key::D))
        {
            self.painter.borrow_mut().painting_mode_enabled = true;
        }
    }

    pub fn show_ui(&mut self, ui: &Ui) {
        let painting_mode = self.painter.borrow().painting_mode_enabled;

        Window::new("Painter")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [10., 10.])
            .show(ui.ctx(), |ui| {
                if painting_mode {
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !self.show_palette {
                        self.painter.borrow_mut().painting_mode_enabled = false;
                    }
                    self.ui_painting(ui);
                } else {
                    self.ui_short(ui);
                }
            });
        if painting_mode {
            self.show_ui_palette(ui);
        }

        self.show_ui_edit(ui);
    }

    pub fn painting_in_progress(&self) -> bool {
        self.painter.borrow().painting_mode_enabled
    }
}

impl Plugin for &MapPainterPlugin {
    fn draw(&self, response: &Response, painter: Painter, projector: &Projector, gesture_handled: bool) {
        let mut mappainter = self.painter.borrow_mut();

        if mappainter.painting_mode_enabled {
            if !mappainter.ignore_painting {
                mappainter.handle_paint(response, projector);
            }

            if gesture_handled {
                mappainter.discard_last_paint();
                mappainter.ignore_painting = true;
            } else if response.drag_released_by(egui::PointerButton::Primary) {
                mappainter.ignore_painting = false;
            }
        }

        mappainter.draw_lines(painter, projector);
    }
}
