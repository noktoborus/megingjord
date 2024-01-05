use egui::Align2;
use egui::Area;
use egui::CentralPanel;
use egui::ComboBox;
use egui::Context;
use egui::Frame;
use egui::Image;
use egui::RichText;
use egui::Ui;
use egui::Window;
use megingjord::terminal::local_osm_tiles::LocalOSMTiles;
use megingjord::terminal::mappainter;
use scanf::sscanf;
use std::collections::HashMap;
use tini::Ini;
use walkers::sources::Attribution;
use walkers::HttpOptions;
use walkers::Map;
use walkers::MapMemory;
use walkers::Position;
use walkers::Tiles;
use walkers::TilesManager;

use std::time;

fn http_options() -> HttpOptions {
    HttpOptions {
        cache: if std::env::var("NO_HTTP_CACHE").is_ok() {
            None
        } else {
            Some(".cache".into())
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Source {
    OpenStreetMap,
    LocalOSMTiles,
}

fn sources(egui_ctx: Context) -> HashMap<Source, Box<dyn TilesManager + Send>> {
    let mut sources: HashMap<Source, Box<dyn TilesManager + Send>> = HashMap::default();

    sources.insert(
        Source::OpenStreetMap,
        Box::new(Tiles::with_options(
            walkers::sources::OpenStreetMap,
            http_options(),
            egui_ctx.to_owned(),
        )),
    );

    sources.insert(Source::LocalOSMTiles, Box::new(LocalOSMTiles::new(egui_ctx.to_owned())));

    sources
}

struct ConfigContext {
    inifile: String,

    lon_lat: Option<Position>,
    zoom: u8,

    trigger_to_save: Option<time::Instant>,
}

pub struct MyApp {
    sources: HashMap<Source, Box<dyn TilesManager + Send>>,
    selected_provider: Source,
    map_memory: MapMemory,
    config_ctx: ConfigContext,
    plugin_painter: mappainter::MapPainterState,
}

impl MyApp {
    pub fn new(egui_ctx: Context) -> Self {
        egui_extras::install_image_loaders(&egui_ctx);

        let config_ctx = ConfigContext {
            inifile: "terminal.ini".to_string(),
            lon_lat: None,
            zoom: 16,
            trigger_to_save: None,
        };

        let mut instance = Self {
            sources: sources(egui_ctx.to_owned()),
            selected_provider: Source::LocalOSMTiles,
            map_memory: MapMemory::default(),
            config_ctx,
            plugin_painter: mappainter::MapPainter::alloc_state(),
        };

        MyApp::config_load(&mut instance);

        instance
    }

    fn config_load(&mut self) {
        let tini = match Ini::from_file(&self.config_ctx.inifile) {
            Ok(tini) => tini,
            Err(err) => {
                println!("Config file {} not loaded: {}", self.config_ctx.inifile, err);
                return;
            }
        };

        self.config_ctx.lon_lat = if let Some(lon_lat_option) = tini.get::<String>("Frame", "lon_lat") {
            let mut lon: f64 = 0.0;
            let mut lat: f64 = 0.0;

            if sscanf!(lon_lat_option.as_str(), "{}, {}", lon, lat).is_ok() {
                Some(Position::from_lon_lat(lon, lat))
            } else {
                None
            }
        } else {
            None
        };

        if let Some(zoom_value) = tini.get("Frame", "zoom") {
            self.config_ctx.zoom = zoom_value;
        }

        if let Some(lon_lat) = self.config_ctx.lon_lat {
            self.map_memory.center_at(lon_lat);
        }

        /* { zoom */
        while self.map_memory.zoom_get() < self.config_ctx.zoom {
            if self.map_memory.zoom_in().is_err() {
                break;
            }
        }

        while self.map_memory.zoom_get() > self.config_ctx.zoom {
            if self.map_memory.zoom_out().is_err() {
                break;
            }
        }
        /* zoom } */
    }

    fn config_update(&mut self) {
        if let Some(position) = self.map_memory.detached() {
            match self.config_ctx.lon_lat {
                Some(lon_lat) => {
                    if lon_lat.lon() != position.lon() || lon_lat.lat() != position.lat() {
                        self.config_ctx.lon_lat = Some(position);
                        self.config_ctx.trigger_to_save = Some(time::Instant::now());
                    }
                }
                None => {
                    self.config_ctx.lon_lat = Some(position);
                    self.config_ctx.trigger_to_save = Some(time::Instant::now());
                }
            }
        } else if self.config_ctx.lon_lat.is_some() {
            self.config_ctx.lon_lat = None;
            self.config_ctx.trigger_to_save = Some(time::Instant::now());
        }

        if self.config_ctx.zoom != self.map_memory.zoom_get() {
            self.config_ctx.zoom = self.map_memory.zoom_get();
            self.config_ctx.trigger_to_save = Some(time::Instant::now());
        }

        if self
            .config_ctx
            .trigger_to_save
            .map_or(false, |instant| instant.elapsed() > time::Duration::from_secs(1))
        {
            let mut tini = Ini::new();

            tini = tini.section("Frame").item("zoom", self.config_ctx.zoom);

            if let Some(lon_lat) = self.config_ctx.lon_lat {
                tini = tini
                    .section("Frame")
                    .item("lon_lat", format!("{}, {}", lon_lat.lon(), lon_lat.lat()));
            }

            match tini.to_file(&self.config_ctx.inifile) {
                Ok(_) => println!("Config file {} saved", self.config_ctx.inifile),
                Err(err) => println!("Config file {} not saved: {}", self.config_ctx.inifile, err),
            }
            self.config_ctx.trigger_to_save = None;
        }
    }
}

pub fn acknowledge(ui: &Ui, attribution: Attribution) {
    Area::new("Acknowledge")
        .anchor(Align2::CENTER_BOTTOM, [0., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if let Some(logo) = attribution.logo_light {
                    ui.add(Image::new(logo).max_height(30.0).max_width(80.0));
                }
                ui.hyperlink_to(attribution.text, attribution.url);
            });
        });
}

/// Simple GUI to zoom in and out.
pub fn zoom(ui: &Ui, map_memory: &mut MapMemory) {
    Window::new("Map")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::LEFT_BOTTOM, [10., -10.])
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button(RichText::new("➕").heading()).clicked() {
                    let _ = map_memory.zoom_in();
                }

                ui.label(format!("{:^3}", map_memory.zoom_get()));

                if ui.button(RichText::new("➖").heading()).clicked() {
                    let _ = map_memory.zoom_out();
                }
            });
        });
}

pub fn controls(ui: &Ui, selected_provider: &mut Source, possible_sources: &mut dyn Iterator<Item = &Source>) {
    Window::new("Satellite")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(Align2::RIGHT_TOP, [-10., 10.])
        .show(ui.ctx(), |ui| {
            ui.collapsing("Map", |ui| {
                ComboBox::from_label("")
                    .selected_text(format!("{:?}", selected_provider))
                    .show_ui(ui, |ui| {
                        ui.set_min_width(100.);
                        for p in possible_sources {
                            ui.selectable_value(selected_provider, *p, format!("{:?}", p));
                        }
                    });
            });
        });
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let rimless = Frame {
            fill: ctx.style().visuals.panel_fill,
            ..Default::default()
        };

        CentralPanel::default().frame(rimless).show(ctx, |ui| {
            let tiles = self.sources.get_mut(&self.selected_provider).unwrap().as_mut();
            let attribution = tiles.attribution();

            // In egui, widgets are constructed and consumed in each frame.
            let map = Map::new(Some(tiles), &mut self.map_memory, Position::from_lon_lat(0.0, 0.0))
                .with_plugin(mappainter::MapPainter::new(&self.plugin_painter));

            ui.add(map);

            // Draw utility windows.
            {
                zoom(ui, &mut self.map_memory);
                controls(ui, &mut self.selected_provider, &mut self.sources.keys());
                acknowledge(ui, attribution);
            }
        });

        self.config_update();
    }
}

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    eframe::run_native(
        "MyApp",
        Default::default(),
        Box::new(|cc| Box::new(MyApp::new(cc.egui_ctx.clone()))),
    )
}
