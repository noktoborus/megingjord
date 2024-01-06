use scanf::sscanf;
use tini::Ini;
use walkers::Position;

#[derive(PartialEq, Clone, Copy)]
pub struct Config {
    pub lat_lon: Option<Position>,
    pub zoom: Option<u8>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            lat_lon: None,
            zoom: None,
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

    pub fn config_load(&mut self) -> Config {
        let tini = match Ini::from_file(&self.inifile) {
            Ok(tini) => tini,
            Err(err) => {
                println!("Config file {} not loaded: {}", self.inifile, err);
                return Config::default();
            }
        };

        let lat_lon = if let Some(lon_lat_option) = tini.get::<String>("Frame", "lat_lon") {
            let mut lon: f64 = 0.0;
            let mut lat: f64 = 0.0;

            if sscanf!(lon_lat_option.as_str(), "{}, {}", lat, lon).is_ok() {
                Some(Position::from_lat_lon(lat, lon))
            } else {
                None
            }
        } else {
            None
        };

        let zoom = tini.get("Frame", "zoom");
        let config = Config { lat_lon, zoom };
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
            let mut tini = Ini::new();

            tini = tini.section("Frame").item("zoom", zoom);

            if let Some(lon_lat) = new_config.lat_lon {
                tini = tini
                    .section("Frame")
                    .item("lon_lat", format!("{}, {}", lon_lat.lon(), lon_lat.lat()));
            }

            match tini.to_file(&self.inifile) {
                Ok(_) => println!("Config file {} saved", self.inifile),
                Err(err) => println!("Config file {} not saved: {}", self.inifile, err),
            }

            self.previous_state = new_config;
        }
    }
}
