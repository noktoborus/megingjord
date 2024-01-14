use super::mappainter::Color;
use egui::{Align2, Painter, Response, Ui, Window};
use geojson::GeoJson;
use walkers::{Plugin, Projector};

#[derive(Debug)]
enum EntryStatus {
    Wait,
    Ready,
    Upload,
    Download,
}

struct Entry {
    id: String,
    json: Option<GeoJson>,
    visible: bool,
    status: EntryStatus,
}

impl Entry {
    fn new_upload(id: String, json: GeoJson) -> Self {
        Self {
            id,
            json: Some(json),
            visible: true,
            status: EntryStatus::Wait,
        }
    }

    fn new_download(id: String) -> Self {
        Self {
            id,
            json: None,
            visible: true,
            status: EntryStatus::Wait,
        }
    }

    pub fn show_ui(&self, ui: &mut Ui) {
        ui.label(format!("{}: {:?}", self.id, self.status));
    }
}

pub struct GeoJsonDispatcher {
    jsons: Vec<Entry>,
}

impl GeoJsonDispatcher {
    pub fn new() -> Self {
        Self { jsons: Vec::new() }
    }

    pub fn download(&mut self, id: String) {
        self.jsons.push(Entry::new_download(id));
    }

    pub fn upload(&mut self, id: String, json: GeoJson) {
        self.jsons.push(Entry::new_upload(id, json));
    }
}

impl Default for GeoJsonDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

fn pair_to_screen_coords(point_pair: &[f64], projector: &Projector) -> egui::Pos2 {
    let x = point_pair[0];
    let y = point_pair[1];

    projector.project(walkers::Position::from_lat_lon(x, y)).to_pos2()
}

impl GeoJsonDispatcher {
    fn draw_linestring(
        &self,
        point_pairs: &[Vec<f64>],
        color: egui::Color32,
        width: f32,
        painter: &Painter,
        projector: &Projector,
    ) {
        let mut iter = point_pairs.iter();

        if let Some(mut previous) = iter.next().map(|x| pair_to_screen_coords(x, projector)) {
            while let Some(last) = iter.next().map(|x| pair_to_screen_coords(x, projector)) {
                painter.line_segment([previous, last], (width, color));
                previous = last;
            }
        }
    }

    fn draw_bbox(&self, _bbox: &geojson::Bbox, _painter: &Painter, _projector: &Projector) {}

    fn draw_feature(&self, feature: &geojson::Feature, painter: &Painter, projector: &Projector) {
        if feature.geometry.is_none() {
            return;
        }

        let extract_props = || {
            if let (Some(color), Some(width)) = (feature.property("color"), feature.property("width")) {
                (
                    (color
                        .as_str()
                        .unwrap()
                        .parse::<Color>()
                        .map_or(None, |x| Some(x.to_color32()))),
                    width.as_f64().map(|x| x as f32),
                )
            } else {
                (None, None)
            }
        };

        if let Some(ref geometry) = feature.geometry {
            if let (Some(color), Some(width)) = extract_props() {
                match geometry.value {
                    geojson::Value::Point(_) => {}
                    geojson::Value::MultiPoint(_) => {}
                    geojson::Value::LineString(ref linestring) => {
                        self.draw_linestring(linestring, color, width, painter, projector)
                    }
                    geojson::Value::MultiLineString(_) => {}
                    geojson::Value::Polygon(_) => {}
                    geojson::Value::MultiPolygon(_) => {}
                    geojson::Value::GeometryCollection(_) => {}
                }
            }
        }
    }

    fn draw_feature_collection(
        &self,
        feature_collection: &geojson::FeatureCollection,
        painter: &Painter,
        projector: &Projector,
    ) {
        if let Some(bbox) = &feature_collection.bbox {
            self.draw_bbox(bbox, painter, projector);
        }

        for feature in &feature_collection.features {
            self.draw_feature(feature, painter, projector);
        }
    }
}

impl Plugin for &GeoJsonDispatcher {
    fn run(&mut self, _response: &Response, painter: Painter, projector: &Projector) {
        for entry in self.jsons.iter() {
            if !entry.visible {
                continue;
            }

            if let Some(json) = &entry.json {
                match json {
                    GeoJson::Geometry(_) => {}
                    GeoJson::Feature(_) => {}
                    GeoJson::FeatureCollection(fc) => self.draw_feature_collection(fc, &painter, projector),
                }
            }
        }
    }
}

impl GeoJsonDispatcher {
    pub fn show_ui(&self, ui: &Ui) {
        if self.jsons.is_empty() {
            return;
        }
        Window::new("Geometry Box")
            .anchor(Align2::RIGHT_TOP, [0., 30.])
            .interactable(true)
            .show(ui.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    self.jsons.iter().for_each(|entry| entry.show_ui(ui));
                })
            });
    }
}
