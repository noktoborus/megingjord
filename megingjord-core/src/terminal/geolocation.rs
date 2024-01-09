use crate::terminal::GeoLocation;

use egui::{Align2, Area, Button, Color32, Painter, Response, RichText, Ui, Vec2, Window};
use geographiclib_rs::{DirectGeodesic, Geodesic};
use walkers::{MapMemory, Plugin, Position, Projector};

pub struct GeoLocationPlugin {
    geolocation: Option<GeoLocation>,
}

const BUTTON_SIZE: egui::Vec2 = egui::Vec2::new(28.0, 28.0);

impl GeoLocationPlugin {
    pub fn new(geolocation: Option<GeoLocation>) -> Self {
        Self { geolocation }
    }

    pub fn show_ui(ui: &Ui, map_memory: &mut MapMemory, geolocation: Option<GeoLocation>, center: Position) {
        if geolocation.is_some() {
            Window::new("GeoLocation")
                .collapsible(false)
                .resizable(false)
                .title_bar(false)
                .anchor(Align2::LEFT_TOP, [64., 10.])
                .show(ui.ctx(), |window_ui| {
                    window_ui.horizontal(|window_ui| {
                        let button = Button::new("↗️");

                        if window_ui.add_sized(BUTTON_SIZE, button).clicked() {
                            map_memory.follow_my_position();
                        }
                        /*
                        window_ui.label(
                            RichText::new(format!(
                                "{:.8}, {:.8}",
                                geolocation.position.lat(),
                                geolocation.position.lon()
                            ))
                            .heading(),
                        );
                        */
                    });
                });
        }
        Area::new("Center position")
            .anchor(Align2::LEFT_TOP, [126., 10.])
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:.6}, {:.6}", center.lat(), center.lon())).heading());
                });
            });
    }
}

impl Plugin for GeoLocationPlugin {
    fn draw(&self, _response: &Response, painter: Painter, projector: &Projector, _gesture_handled: bool) {
        let wgs84 = Geodesic::wgs84();

        if let Some(geolocation) = self.geolocation {
            let center = projector.reverse(Vec2 { x: 0.0, y: 0.0 });
            let position = projector.project(geolocation.position);
            let (accuracy_lat, accuracy_lon, _) =
                wgs84.direct(center.lat(), center.lon(), 0.0, geolocation.accuracy.into());
            let accuracy_position = projector
                .project(Position::from_lat_lon(accuracy_lat, accuracy_lon))
                .to_pos2();
            let radius = accuracy_position.distance(projector.project(center).to_pos2());

            painter.circle_filled(position.to_pos2(), radius, Color32::BLACK.gamma_multiply(0.1));
            painter.circle_filled(position.to_pos2(), 4., Color32::BLACK);
            painter.circle_filled(position.to_pos2(), 2.5, Color32::YELLOW);
        }
    }
}
