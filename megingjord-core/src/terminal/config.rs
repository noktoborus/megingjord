#[cfg(target_arch = "wasm32")]
use log;
use scanf::sscanf;
#[cfg(not(target_arch = "wasm32"))]
use tini::Ini;
use walkers::Position;
#[cfg(target_arch = "wasm32")]
use web_sys;

#[derive(PartialEq, Clone, Copy, Default)]
pub struct Config {
    pub lat_lon: Option<Position>,
    pub zoom: Option<u8>,
}

pub struct ConfigContext {
    inifile: String,
    previous_state: Config,
    saver_guard: u32,

    #[cfg(target_arch = "wasm32")]
    local_storage: Option<web_sys::Storage>,
}

fn str_to_position(lat_lon_option: String) -> Option<Position> {
    let mut lon: f64 = 0.0;
    let mut lat: f64 = 0.0;

    if sscanf!(lat_lon_option.as_str(), "{}, {}", lat, lon).is_ok() {
        Some(Position::from_lat_lon(lat, lon))
    } else {
        None
    }
}

fn position_to_str(lat_lon: &Position) -> String {
    format!("{}, {}", lat_lon.lat(), lat_lon.lon())
}

const SAVER_GUARD_VALUE: u32 = 60;

impl ConfigContext {
    pub fn new(config_name: String) -> Self {
        Self {
            inifile: config_name,
            previous_state: Config::default(),
            saver_guard: SAVER_GUARD_VALUE,
            #[cfg(target_arch = "wasm32")]
            local_storage: match web_sys::window() {
                Some(window) => match window.local_storage() {
                    Ok(ls) => ls,
                    Err(err) => {
                        log::error!("websys: LocalStorage not acquired: {:?}", err);
                        None
                    }
                },
                None => {
                    log::error!("websys: window not acquired");
                    None
                }
            },
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn config_read(&mut self) -> Config {
        let tini = match Ini::from_file(&self.inifile) {
            Ok(tini) => tini,
            Err(err) => {
                println!("Config file {} not loaded: {}", self.inifile, err);
                return Config::default();
            }
        };

        let lat_lon = if let Some(lat_lon_option) = tini.get::<String>("Frame", "lat_lon") {
            str_to_position(lat_lon_option)
        } else {
            None
        };

        let zoom = tini.get("Frame", "zoom");
        Config { lat_lon, zoom }
    }

    pub fn config_load(&mut self) -> Config {
        let config = self.config_read();
        log::info!("loading config: {}", self.inifile);

        self.previous_state = config;
        config
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn config_write(&self, new_config: &Config) {
        let mut tini = Ini::new();

        tini = tini.section("Frame").item("zoom", new_config.zoom.unwrap());

        if let Some(lat_lon) = new_config.lat_lon {
            tini = tini.section("Frame").item("lat_lon", position_to_str(&lat_lon));
        }

        match tini.to_file(&self.inifile) {
            Ok(_) => println!("Config file {} saved", self.inifile),
            Err(err) => println!("Config file {} not saved: {}", self.inifile, err),
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn config_read(&mut self) -> Config {
        if let Some(local_storage) = &self.local_storage {
            let lat_lon = match local_storage.get_item("lat_lon") {
                Ok(val) => {
                    if let Some(lat_lon) = val {
                        str_to_position(lat_lon)
                    } else {
                        None
                    }
                }
                Err(err) => {
                    log::error!("'zoom' key not loaded: {:?}", err);
                    None
                }
            };

            let zoom = match local_storage.get_item("zoom") {
                Ok(val) => {
                    if let Some(zoom_val) = val {
                        match zoom_val.parse::<u8>() {
                            Ok(zoom) => Some(zoom),
                            Err(err) => {
                                log::error!("'zoom' value '{}' not parsed: {:?}", zoom_val, err);
                                None
                            }
                        }
                    } else {
                        None
                    }
                }
                Err(err) => {
                    log::error!("'zoom' key not loaded: {:?}", err);
                    None
                }
            };
            Config { lat_lon, zoom }
        } else {
            Config::default()
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn config_write(&self, new_config: &Config) {
        if let Some(local_storage) = &self.local_storage {
            if let Some(zoom) = new_config.zoom {
                local_storage
                    .set_item("zoom", format!("{}", zoom).as_str())
                    .unwrap_or({});
            } else {
                local_storage.delete("zoom").unwrap_or({});
            }

            if let Some(lat_lon) = new_config.lat_lon {
                local_storage
                    .set_item("lat_lon", &position_to_str(&lat_lon))
                    .unwrap_or({});
            } else {
                local_storage.delete("lat_lon").unwrap_or({});
            }
        }
    }

    pub fn config_update(&mut self, zoom: u8, lat_lon: Option<Position>) {
        let new_config = Config {
            lat_lon,
            zoom: Some(zoom),
        };

        if self.saver_guard == 0 {
            self.saver_guard = SAVER_GUARD_VALUE;
        } else {
            self.saver_guard -= 1;
        }

        if self.saver_guard == 0 && new_config != self.previous_state {
            self.config_write(&new_config);
            self.previous_state = new_config;
        }
    }
}
