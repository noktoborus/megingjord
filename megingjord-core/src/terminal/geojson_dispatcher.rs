use super::mappainter::Color;
use egui::{Align2, Painter, Response, RichText, Ui, Window};
use geojson::GeoJson;
use std::sync::{Arc, RwLock};
use walkers::{Plugin, Projector};

use reqwest::{header, Client, StatusCode};

struct Task {}

impl Task {
    pub fn download(client: Client, local_id: u32, entries: &Arc<RwLock<Vec<Entry>>>, jsonid: String) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let entries = Arc::clone(entries);

            std::thread::spawn(move || {
                runtime.block_on(async { Task::run_download(client, local_id, entries, jsonid).await })
            });
        }

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move { Task::run_download(client, local_id, entries, jsonid).await });

        Self {}
    }

    pub fn upload(client: Client, local_id: u32, entries: &Arc<RwLock<Vec<Entry>>>) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            let entries = Arc::clone(entries);

            std::thread::spawn(move || runtime.block_on(async { Task::run_upload(client, local_id, entries).await }));
        }

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move { Task::run_upload(client, local_id, entries).await });

        Self {}
    }

    async fn run_download(client: Client, local_id: u32, entries: Arc<RwLock<Vec<Entry>>>, jsonid: String) {
        entries
            .write()
            .unwrap()
            .iter_mut()
            .find(|entry| entry.local_id == local_id)
            .map(|entry| {
                entry.status = EntryStatus::Downloading;
            });

        let result = match client.get(format!("http://127.0.0.1:3000/get/{}", jsonid)).send().await {
            Ok(response) => {
                if response.status() == StatusCode::OK {
                    Ok(response.json::<GeoJson>().await.unwrap())
                } else {
                    Err(format!("server returns code {}", response.status()))
                }
            }
            Err(err) => Err(format!("generic error: {}", err)),
        };

        entries
            .write()
            .unwrap()
            .iter_mut()
            .find(|entry| entry.local_id == local_id)
            .map(|entry| match result {
                Ok(geojson) => {
                    entry.status = EntryStatus::Ready(entry.id.clone());
                    entry.json = Some(geojson);
                }
                Err(error) => {
                    entry.status = EntryStatus::DownloadError(error);
                }
            });
    }

    async fn run_upload(client: Client, local_id: u32, entries: Arc<RwLock<Vec<Entry>>>) {
        let json_body = entries
            .write()
            .unwrap()
            .iter_mut()
            .find(|entry| entry.local_id == local_id)
            .map(|entry| {
                entry.status = EntryStatus::Uploading;
                entry.json.as_ref().unwrap().to_string()
            });

        let status = if let Some(json_body) = json_body {
            let response = client
                .post("http://127.0.0.1:3000/new")
                .header(header::CONTENT_TYPE, "application/geo+json")
                .body(json_body)
                .send()
                .await;

            match response {
                Ok(response) => {
                    if response.status() == StatusCode::OK {
                        match response.text().await {
                            Ok(identifier) => EntryStatus::Ready(identifier),
                            Err(err) => EntryStatus::UploadError(format!("{}", err)),
                        }
                    } else {
                        EntryStatus::UploadError(format!("server error code: {}", response.status()))
                    }
                }
                Err(err) => EntryStatus::UploadError(format!("{}", err)),
            }
        } else {
            EntryStatus::UploadError(format!("Nothing to upload: body is empty"))
        };

        match status {
            EntryStatus::Ready(identifier) => {
                if let Some(entry) = entries
                    .write()
                    .unwrap()
                    .iter_mut()
                    .find(|entry| entry.local_id == local_id)
                {
                    entry.status = EntryStatus::Ready(identifier.clone());
                    entry.id = identifier;
                }
            }
            _ => {
                if let Some(entry) = entries
                    .write()
                    .unwrap()
                    .iter_mut()
                    .find(|entry| entry.local_id == local_id)
                {
                    entry.status = status;
                }
            }
        }
    }
}

#[derive(Debug, Default)]
enum EntryStatus {
    #[default]
    Wait,
    Ready(String),
    Downloading,
    DownloadError(String),
    Uploading,
    UploadError(String),
}

struct Entry {
    local_id: u32,
    id: String,
    json: Option<GeoJson>,
    visible: bool,
    status: EntryStatus,
}

impl Entry {
    fn new_with_id(local_id: u32, id: String) -> Self {
        Self {
            local_id,
            id,
            json: None,
            visible: true,
            status: Default::default(),
        }
    }

    fn new_with_json(local_id: u32, json: GeoJson) -> Self {
        Self {
            local_id,
            id: "".to_string(),
            json: Some(json.clone()),
            visible: true,
            status: Default::default(),
        }
    }

    pub fn show_ui(&mut self, ui: &mut Ui) {
        ui.checkbox(
            &mut self.visible,
            RichText::new(format!("{}: {:?}", self.id, self.status)).heading(),
        );
    }
}

pub struct GeoJsonDispatcher {
    entries: Arc<RwLock<Vec<Entry>>>,
    client: Client,
    id_generator: u32,
}

impl GeoJsonDispatcher {
    fn next_id(&mut self) -> u32 {
        self.id_generator += 1;
        self.id_generator
    }

    pub fn new() -> Self {
        Self {
            entries: Default::default(),
            client: Default::default(),
            id_generator: 1,
        }
    }

    pub fn download(&mut self, id: String) {
        let local_id = self.next_id();

        self.entries
            .write()
            .unwrap()
            .push(Entry::new_with_id(local_id, id.clone()));
        Task::download(self.client.clone(), local_id, &self.entries, id);
    }

    pub fn upload_json_array(&mut self, jsons: &mut Vec<geojson::GeoJson>) {
        while let Some(json) = jsons.pop() {
            let local_id = self.next_id();
            self.entries.write().unwrap().push(Entry::new_with_json(local_id, json));
            Task::upload(self.client.clone(), local_id, &self.entries);
        }
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
        for entry in self.entries.read().unwrap().iter() {
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
    pub fn show_ui(&mut self, ui: &Ui) {
        if self.entries.read().unwrap().is_empty() {
            return;
        }
        Window::new("")
            .anchor(Align2::RIGHT_TOP, [-10., 30.])
            .interactable(true)
            .show(ui.ctx(), |ui| {
                self.entries
                    .write()
                    .unwrap()
                    .iter_mut()
                    .for_each(|entry| entry.show_ui(ui));
            });
    }
}
