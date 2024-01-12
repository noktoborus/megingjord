use egui::{Align2, Painter, Response, RichText, Ui, Window};
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
    current: DrawedLine,
    completed: Vec<DrawedLine>,
    painting_mode_enabled: bool,
    ignore_painting: bool,
}

impl MapPainter {
    fn new() -> Self {
        Self {
            current: Default::default(),
            completed: Vec::new(),
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

    fn palette_ui(&mut self, ui: &mut Ui) {
        let colors_and_keys = [
            (egui::Color32::RED, egui::Key::Num1),
            (egui::Color32::BLUE, egui::Key::Num2),
            (egui::Color32::GREEN, egui::Key::Num3),
            (egui::Color32::BROWN, egui::Key::Num4),
        ];

        for (color, key) in colors_and_keys.iter() {
            let color_button = egui::Button::new(format!("{}", key.name())).fill(*color);

            if ui
                .add_sized(BUTTON_SIZE, color_button)
                .on_hover_text(format!("Shortcut: {}", key.name()))
                .clicked()
                || ui.input(|i| i.key_pressed(*key))
            {
                self.active_color = *color;
                self.painter.borrow_mut().set_color(*color);
                self.show_palette = false;
            }
        }
    }

    fn show_palette(&mut self, window_ui: &Ui) {
        Window::new("Palette")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [54.0, 60.0])
            .show(window_ui.ctx(), |ui| {
                self.palette_ui(ui);

                if window_ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.show_palette = false;
                }
            });
    }

    fn ui_edit(&mut self, ui: &mut Ui) {
        if ui
            .add_sized(BUTTON_SIZE, egui::Button::new(RichText::new("🗙").heading()))
            .on_hover_text("Cancel painting\nShortcut: D or Escape")
            .clicked()
            || ui.input(|i| i.key_pressed(egui::Key::D))
        {
            self.painter.borrow_mut().painting_mode_enabled = false;
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
            self.show_palette = false;
        }
    }

    pub fn show_ui(&mut self, ui: &Ui) {
        Window::new("Painter")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [10., 10.])
            .show(ui.ctx(), |window_ui| {
                if self.painter.borrow().painting_mode_enabled {
                    if window_ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        if !self.show_palette {
                            self.painter.borrow_mut().painting_mode_enabled = false;
                        }
                    }

                    self.ui_edit(window_ui);
                    if self.show_palette {
                        self.show_palette(ui);
                    }
                } else {
                    self.ui_short(window_ui);
                }
            });
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
