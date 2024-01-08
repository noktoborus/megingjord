use egui::{Align2, Painter, Response, RichText, Ui, Window};
use std::cell::RefCell;
use std::rc::Rc;
use walkers::{Plugin, Position, Projector};

struct MapPainter {
    current: Vec<Position>,
    completed: Vec<Vec<Position>>,
    painting_mode_enabled: bool,
    ignore_painting: bool,
}

impl MapPainter {
    fn new() -> Self {
        Self {
            current: Vec::new(),
            completed: Vec::new(),
            painting_mode_enabled: false,
            ignore_painting: false,
        }
    }
}

impl MapPainter {
    fn handle_paint(&mut self, response: &Response, projector: &Projector) {
        if response.clicked_by(egui::PointerButton::Primary) {
            println!("clicled");
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(offset) = response
                .hover_pos()
                .map(|x| projector.reverse(x - response.rect.center()))
            {
                self.current.push(offset);
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
        {
            let mut points = self.current.iter();
            if let Some(first) = points.next() {
                let mut prev_point = first;
                for point in points {
                    painter.line_segment(
                        [
                            projector.project(*prev_point).to_pos2(),
                            projector.project(*point).to_pos2(),
                        ],
                        (2., egui::Color32::RED),
                    );
                    prev_point = point;
                }
            }
        }

        {
            for line in self.completed.iter() {
                let mut points = line.iter();
                if let Some(first) = points.next() {
                    let mut prev_point = first;
                    for point in points {
                        painter.line_segment(
                            [
                                projector.project(*prev_point).to_pos2(),
                                projector.project(*point).to_pos2(),
                            ],
                            (2., egui::Color32::BLUE),
                        );
                        prev_point = point;
                    }
                }
            }
        }
    }
}

pub struct MapPainterPlugin {
    painter: Rc<RefCell<MapPainter>>,
}

impl MapPainterPlugin {
    pub fn new() -> Self {
        Self {
            painter: Rc::new(RefCell::new(MapPainter::new())),
        }
    }

    fn ui_edit(&self, ui: &mut Ui) {
        if ui.button(RichText::new("🗙").heading()).clicked() {
            self.painter.borrow_mut().painting_mode_enabled = false;
        }
    }

    fn ui_short(&self, ui: &mut Ui) {
        if ui.button(RichText::new("📓").heading()).clicked() {
            self.painter.borrow_mut().painting_mode_enabled = true;
        }
    }

    pub fn show_ui(&self, ui: &Ui) {
        Window::new("Painter")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [20., 20.])
            .show(ui.ctx(), |ui| {
                if self.painter.borrow().painting_mode_enabled {
                    self.ui_edit(ui);
                } else {
                    self.ui_short(ui);
                }

                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.painter.borrow_mut().painting_mode_enabled = false;
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
