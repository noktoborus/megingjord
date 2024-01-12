use scanf::sscanf;
use std::fmt::Display;
use std::str::FromStr;

#[derive(PartialEq, Clone, Copy, Default)]
pub struct Config {
    pub lat_lon: Option<Position>,
    pub zoom: Option<u8>,
}

#[derive(PartialEq, Clone, Copy, Default)]
pub struct Position {
    lat: f64,
    lon: f64,
}

impl Position {
    pub fn from_position(other: walkers::Position) -> Self {
        Self {
            lat: other.lat(),
            lon: other.lon(),
        }
    }

    pub fn to_position(&self) -> walkers::Position {
        walkers::Position::from_lat_lon(self.lat, self.lon)
    }
}

impl Display for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, {}", self.lat, self.lon)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PositionParseError;

impl FromStr for Position {
    type Err = PositionParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut lat: f64 = 0.0;
        let mut lon: f64 = 0.0;

        sscanf!(s, "{}, {}", lat, lon).map_err(|_| PositionParseError)?;

        Ok(Self { lat, lon })
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct ConfigReadWriter {
    tini: Option<tini::Ini>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ConfigReadWriter {
    fn new() -> Self {
        Self {
            tini: Some(tini::Ini::new()),
        }
    }

    fn read(path: &str) -> Self {
        Self {
            tini: match tini::Ini::from_file(path) {
                Ok(tini) => Some(tini),
                Err(err) => {
                    log::info!("Config file {} not loaded: {}", path, err);
                    None
                }
            },
        }
    }

    fn write(&self, path: &str) {
        if let Some(tini) = self.tini.as_ref() {
            match tini.to_file(path) {
                Ok(_) => log::info!("Config file {} saved", path),
                Err(err) => log::error!("Config file {} not saved: {}", path, err),
            }
        };
    }

    pub fn set<V>(self, key: &str, value: Option<V>) -> Self
    where
        V: Display,
    {
        Self {
            tini: if let Some(tini) = self.tini {
                if let Some(value) = value {
                    Some(tini.section("all").item(key, value))
                } else {
                    Some(tini)
                }
            } else {
                None
            },
        }
    }

    fn get<T>(&self, key: &str) -> Option<T>
    where
        T: FromStr,
    {
        self.tini.as_ref().and_then(|x| x.get("all", key))
    }
}

#[cfg(target_arch = "wasm32")]
struct ConfigReadWriter {
    local_storage: Option<web_sys::Storage>,
}

#[cfg(target_arch = "wasm32")]
impl ConfigReadWriter {
    fn new() -> Self {
        Self {
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

    fn read(_path: &str) -> Self {
        Self::new()
    }

    fn write(&self, _path: &str) {}

    pub fn set<V>(self, key: &str, value: Option<V>) -> Self
    where
        V: Display,
    {
        Self {
            local_storage: if let Some(local_storage) = &self.local_storage {
                if let Some(value) = value {
                    local_storage.set_item(key, format!("{}", value).as_str()).unwrap_or({});
                } else {
                    local_storage.delete(key).unwrap_or({});
                }
                Some(local_storage.clone())
            } else {
                None
            },
        }
    }

    fn get<T>(&self, key: &str) -> Option<T>
    where
        T: FromStr,
    {
        if let Some(local_storage) = &self.local_storage {
            match local_storage.get_item(key) {
                Ok(val) => val.and_then(|x| x.parse().ok()),
                Err(err) => {
                    log::error!("'{}' key not loaded: {:?}", key, err);
                    None
                }
            }
        } else {
            None
        }
    }
}

pub struct ConfigContext {
    inifile: String,
    previous_state: Config,
    saver_guard: u32,
}

const SAVER_GUARD_VALUE: u32 = 60;

impl ConfigContext {
    pub fn new(config_name: String) -> Self {
        Self {
            inifile: config_name,
            previous_state: Config::default(),
            saver_guard: SAVER_GUARD_VALUE,
        }
    }

    pub fn config_read(&mut self) -> Config {
        let reader = ConfigReadWriter::read(&self.inifile);

        Config {
            lat_lon: reader.get("lat_lon"),
            zoom: reader.get("zoom"),
        }
    }

    pub fn config_load(&mut self) -> Config {
        log::info!("loading config: {}", self.inifile);

        let reader = ConfigReadWriter::read(&self.inifile);
        let config = Config {
            lat_lon: reader.get("lat_lon"),
            zoom: reader.get("zoom"),
        };

        self.previous_state = config;
        config
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
            ConfigReadWriter::new()
                .set("zoom", new_config.zoom)
                .set("lat_lon", new_config.lat_lon)
                .write(&self.inifile);

            self.previous_state = new_config;
        }
    }
}
