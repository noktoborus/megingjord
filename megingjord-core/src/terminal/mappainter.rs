use egui::{Align2, Area, Color32, Key, Painter, Response, RichText, Ui, Window};
use scanf::sscanf;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;
use walkers::{Plugin, Projector};

#[derive(Clone, Serialize, Deserialize)]
struct Point(f64, f64);

impl Point {
    fn to_position(&self) -> walkers::Position {
        walkers::Position::from_lat_lon(self.0, self.1)
    }

    fn from_position(other: walkers::Position) -> Self {
        Self(other.lat(), other.lon())
    }
}

#[derive(Clone, Default, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

impl Color {
    fn from_color32(other: egui::Color32) -> Self {
        Self {
            r: other.r(),
            g: other.g(),
            b: other.b(),
        }
    }
    fn to_color32(self) -> egui::Color32 {
        egui::Color32::from_rgb(self.r, self.g, self.b)
    }
}

impl Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ColorParseError;

impl Display for ColorParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::str::FromStr for Color {
    type Err = ColorParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut r: u8 = 0;
        let mut g: u8 = 0;
        let mut b: u8 = 0;

        sscanf!(s, "#{:x}{:x}{:x}", r, g, b).map_err(|_| ColorParseError)?;

        Ok(Self { r, g, b })
    }
}

impl Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct DrawedLine {
    color: Color,
    points: Vec<Point>,
}

impl Default for DrawedLine {
    fn default() -> Self {
        Self {
            points: Vec::new(),
            color: Color::from_color32(egui::Color32::RED),
        }
    }
}

impl DrawedLine {
    fn clear(&mut self) {
        self.points.clear()
    }
}

#[derive(Serialize, Deserialize)]
struct PainterLines {
    /// Completed lines
    completed: Vec<DrawedLine>,
    /// Forward history
    forward_history: Vec<DrawedLine>,
}

struct MapPainter {
    /// Current line.
    /// Move points to completed at first 'drag_released' event
    current: DrawedLine,
    lines: PainterLines,
    painting_mode_enabled: bool,
    ignore_painting: bool,
}

impl MapPainter {
    fn apply_state_json(state_json: Option<String>) -> PainterLines {
        if let Some(state_json) = state_json {
            match serde_json::from_str(&state_json) {
                Ok(state) => {
                    return state;
                }
                Err(err) => println!("Deserialize MapPainter state error: {:?}", err),
            }
        }

        PainterLines {
            completed: Vec::new(),
            forward_history: Vec::new(),
        }
    }

    fn new(state_json: Option<String>) -> Self {
        Self {
            current: Default::default(),
            lines: Self::apply_state_json(state_json),
            painting_mode_enabled: false,
            ignore_painting: false,
        }
    }
}

impl MapPainter {
    fn set_color(&mut self, color: Color) {
        self.current.color = color;
    }

    fn handle_paint(&mut self, response: &Response, projector: &Projector) {
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(offset) = response
                .hover_pos()
                .map(|x| projector.reverse(x - response.rect.center()))
            {
                self.current.points.push(Point::from_position(offset));
            }
        }

        if response.drag_released_by(egui::PointerButton::Primary) {
            self.lines.completed.push(self.current.clone());
            self.current.clear();
            self.lines.forward_history.clear();
        }
    }

    fn discard_last_paint(&mut self) {
        self.current.clear();
    }

    fn draw_lines(&self, painter: Painter, projector: &Projector) {
        for line in [self.lines.completed.iter(), [self.current.clone()].iter()]
            .into_iter()
            .flatten()
        {
            let mut points = line.points.iter();
            if let Some(first) = points.next() {
                let mut prev_point = first;
                for point in points {
                    painter.line_segment(
                        [
                            projector.project(prev_point.to_position()).to_pos2(),
                            projector.project(point.to_position()).to_pos2(),
                        ],
                        (2.5, line.color.to_color32()),
                    );
                    prev_point = point;
                }
            }
        }
    }

    /// Undo last drawed line
    fn undo_line(&mut self) {
        if let Some(last_line) = self.lines.completed.pop() {
            self.lines.forward_history.push(last_line);
        }
    }

    fn redo_line(&mut self) {
        if let Some(next_line) = self.lines.forward_history.pop() {
            self.lines.completed.push(next_line);
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
        Self::new(None)
    }
}

const BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(28.0, 28.0);
const SPACER_SIZE: f32 = 16.0;

impl MapPainterPlugin {
    pub fn new(state_json: Option<String>) -> Self {
        Self {
            painter: Rc::new(RefCell::new(MapPainter::new(state_json))),
            active_color: egui::Color32::RED,
            show_palette: false,
        }
    }

    pub fn get_state_json(&self) -> Option<String> {
        let painter = self.painter.borrow();

        match serde_json::to_string(&painter.lines) {
            Ok(json_string) => Some(json_string),
            Err(err) => {
                log::error!("Painter serialization problem: {:?}", err);
                None
            }
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
                    self.painter.borrow_mut().set_color(Color::from_color32(*color));
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
                    self.painter.borrow_mut().set_color(Color::from_color32(*color));
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
                !painter.lines.completed.is_empty(),
                !painter.lines.forward_history.is_empty(),
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
