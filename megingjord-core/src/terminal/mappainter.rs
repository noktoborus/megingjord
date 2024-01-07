use egui::{Align2, Painter, Response, RichText, Ui, Window};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use walkers::{Plugin, Position, Projector};

pub struct MapPainterData {
    current: RefCell<Vec<Position>>,
    completed: RefCell<Vec<Vec<Position>>>,
    painting_mode_enabled: Cell<bool>,
    ignore_painting: Cell<bool>,
}

pub struct MapPainterUi {
    data: Rc<MapPainterData>,
}

impl MapPainterUi {
    pub fn new() -> Self {
        MapPainterUi {
            data: Rc::new(MapPainterData {
                current: RefCell::new(Vec::new()),
                completed: RefCell::new(Vec::new()),
                painting_mode_enabled: Cell::new(false),
                ignore_painting: Cell::new(false),
            }),
        }
    }

    pub fn painting_in_progress(&self) -> bool {
        self.data.painting_mode_enabled.get()
    }

    fn show_edit(&self, ui: &Ui) {
        Window::new("Painter")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [20., 20.])
            .show(ui.ctx(), |ui| {
                if ui.button(RichText::new("🗙").heading()).clicked() {
                    self.data.painting_mode_enabled.set(false);
                }
            });
    }

    pub fn show_short(&self, ui: &Ui) {
        Window::new("Painter")
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .anchor(Align2::LEFT_TOP, [20., 20.])
            .show(ui.ctx(), |ui| {
                if ui.button(RichText::new("📓").heading()).clicked() {
                    self.data.painting_mode_enabled.set(true);
                }
            });
    }

    pub fn show_ui(&self, ui: &Ui) {
        if self.data.painting_mode_enabled.get() {
            self.show_edit(ui);
        } else {
            self.show_short(ui);
        }
    }
}

pub struct MapPainter {
    state: Rc<MapPainterData>,
}

impl MapPainter {
    pub fn new(state: &MapPainterUi) -> Self {
        Self {
            state: Rc::clone(&state.data),
        }
    }

    fn handle_paint(&self, response: &Response, projector: &Projector) {
        if response.clicked_by(egui::PointerButton::Primary) {
            println!("clicled");
        }

        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(offset) = response
                .hover_pos()
                .map(|x| projector.reverse(x - response.rect.center()))
            {
                self.state.current.borrow_mut().push(offset);
            }
        }

        if response.drag_released_by(egui::PointerButton::Primary) {
            {
                let current_bind = self.state.current.borrow();
                let mut completed_bind = self.state.completed.borrow_mut();
                completed_bind.push(current_bind.clone());
            }
            {
                self.state.current.borrow_mut().clear();
            }
        }
    }

    fn discard_last_paint(&self) {
        let mut current_bind = self.state.current.borrow_mut();
        current_bind.clear();
    }

    fn draw_lines(&self, painter: Painter, projector: &Projector) {
        {
            let binding = self.state.current.borrow();
            let mut points = binding.iter();
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
            let binding = self.state.completed.borrow();
            for line in binding.iter() {
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

impl Plugin for MapPainter {
    fn draw(&self, response: &Response, painter: Painter, projector: &Projector, gesture_handled: bool) {
        if self.state.painting_mode_enabled.get() {
            if !self.state.ignore_painting.get() {
                self.handle_paint(response, projector);
            }

            if gesture_handled {
                self.discard_last_paint();
                self.state.ignore_painting.set(true);
            } else if response.drag_released_by(egui::PointerButton::Primary) {
                self.state.ignore_painting.set(false);
            }
        }

        self.draw_lines(painter, projector);
    }
}
