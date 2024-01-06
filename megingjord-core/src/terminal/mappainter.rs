use egui::{Painter, Response};
use std::cell::RefCell;
use std::rc::Rc;
use walkers::{Plugin, Position, Projector};

pub struct MapPainterData {
    current: RefCell<Vec<Position>>,
    completed: RefCell<Vec<Vec<Position>>>,
}

pub type MapPainterState = Rc<MapPainterData>;

pub struct MapPainter {
    state: MapPainterState,
}

impl MapPainter {
    pub fn alloc_state() -> MapPainterState {
        Rc::new(MapPainterData {
            current: RefCell::new(Vec::new()),
            completed: RefCell::new(Vec::new()),
        })
    }

    pub fn new(resource: &MapPainterState) -> Self {
        Self {
            state: Rc::clone(resource),
        }
    }
}

impl Plugin for MapPainter {
    fn draw(&self, response: &Response, painter: Painter, projector: &Projector) {
        if response.dragged_by(egui::PointerButton::Secondary) {
            if let Some(offset) = response
                .hover_pos()
                .map(|x| projector.reverse(x - response.rect.center()))
            {
                self.state.current.borrow_mut().push(offset);
            }
        }

        if response.drag_released_by(egui::PointerButton::Secondary) {
            {
                let current_bind = self.state.current.borrow();
                let mut completed_bind = self.state.completed.borrow_mut();
                completed_bind.push(current_bind.clone());
            }
            {
                self.state.current.borrow_mut().clear();
            }
        }

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
