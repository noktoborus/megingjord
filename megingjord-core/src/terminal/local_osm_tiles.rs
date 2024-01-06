use egui::Context;
use renderer::draw::drawer::Drawer;
use renderer::draw::tile_pixels::TilePixels;
use renderer::geodata::reader::GeodataReader;
use renderer::geodata::reader::OsmEntities;
use renderer::mapcss::parser::parse_file;
use renderer::mapcss::styler::StyledEntities;
use renderer::mapcss::styler::Styler;
use renderer::tile::tile::Tile;
use renderer::tile::tile::TILE_SIZE;
use std::collections::hash_map;
use std::collections::HashMap;
use std::include_bytes;
use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::Arc;
use std::thread;
use std::vec::Vec;
use walkers::sources::Attribution;
use walkers::Texture;
use walkers::TileId;
use walkers::TilesManager;

use std::sync::mpsc;

enum TextureState {
    None,
    Collecting,
    Styling,
    Draw,
    Done { texture: Texture },
    /* Tile has no data */
    Empty,
}

struct RenderContext<'a> {
    egui_ctx: Context,
    styler: Styler,
    drawer: Drawer,
    reader: GeodataReader<'a>,
    scale: usize,
}

enum ThreadCommand {
    Draw { tile_id: TileId },
    Terminate,
}

enum ThreadResponse {
    Collecting { tile_id: TileId },
    Styling { tile_id: TileId },
    Draw { tile_id: TileId },
    Done { tile_id: TileId, texture: Texture },
    /* No OSM data in this tile */
    Empty { tile_id: TileId },
}

struct ThreadContext {
    no: usize,
    handler: thread::JoinHandle<()>,
    cmd_tx: mpsc::Sender<ThreadCommand>,
    texture_rx: mpsc::Receiver<ThreadResponse>,
}

struct ThreadsContext {
    contexts: Vec<ThreadContext>,
    threads_free: Vec<usize>,
}

impl ThreadsContext {
    fn new(render_ctx: Arc<RenderContext<'static>>) -> Self {
        let threads_count = thread::available_parallelism().unwrap_or(NonZeroUsize::new(1).unwrap());

        println!("Start {} render threads", threads_count);

        let mut ctx = Self {
            contexts: Vec::new(),
            threads_free: Vec::new(),
        };

        for _ in 0..threads_count.into() {
            ThreadsContext::thread_spawn(&mut ctx, &render_ctx);
        }

        ctx
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.threads_free.len() == self.contexts.len()
    }

    pub(crate) fn is_busy(&self) -> bool {
        self.threads_free.is_empty()
    }

    pub(crate) fn draw_request(&mut self, tile_id: TileId) {
        if !self.threads_free.is_empty() {
            let thread_no = self.threads_free.pop().unwrap();

            self.contexts[thread_no]
                .cmd_tx
                .send(ThreadCommand::Draw { tile_id })
                .unwrap();
        }
    }

    pub(crate) fn response_collect(&mut self) -> Vec<ThreadResponse> {
        let mut messages = Vec::new();

        for context in &self.contexts {
            if let Ok(message) = context.texture_rx.try_recv() {
                match message {
                    ThreadResponse::Collecting { tile_id: _ } => {}
                    ThreadResponse::Draw { tile_id: _ } => {}
                    ThreadResponse::Done { tile_id: _, texture: _ } => self.threads_free.push(context.no),
                    ThreadResponse::Empty { tile_id: _ } => self.threads_free.push(context.no),
                    ThreadResponse::Styling { tile_id: _ } => {}
                }
                messages.push(message)
            }
        }

        messages
    }

    fn thread_spawn(&mut self, render_ctx: &Arc<RenderContext<'static>>) {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (texture_tx, texture_rx) = mpsc::channel();
        let render_ctx_ref = Arc::clone(render_ctx);
        let thread_builder = thread::Builder::new().name(format!("Render {}", self.contexts.len()));

        let context = ThreadContext {
            no: self.contexts.len(),
            handler: thread_builder
                .spawn(move || ThreadsContext::thread_main(cmd_rx, texture_tx, render_ctx_ref))
                .unwrap(),
            cmd_tx,
            texture_rx,
        };

        self.threads_free.push(self.contexts.len());
        self.contexts.push(context);
    }

    fn thread_main(
        rx: mpsc::Receiver<ThreadCommand>,
        tx: mpsc::Sender<ThreadResponse>,
        render_ctx: Arc<RenderContext>,
    ) {
        while let Ok(msg) = rx.recv() {
            match msg {
                ThreadCommand::Terminate => {
                    println!("thread: Terminate message received");
                    break;
                }
                ThreadCommand::Draw { tile_id } => {
                    tx.send(ThreadResponse::Collecting { tile_id }).unwrap();
                    let entities = render_ctx.collect_tile(tile_id);
                    if entities.is_empty() {
                        tx.send(ThreadResponse::Empty { tile_id }).unwrap();
                    } else {
                        tx.send(ThreadResponse::Styling { tile_id }).unwrap();
                        let styled = render_ctx.collect_styled(&tile_id, &entities);

                        if styled.is_empty() {
                            tx.send(ThreadResponse::Empty { tile_id }).unwrap();
                        } else {
                            tx.send(ThreadResponse::Draw { tile_id }).unwrap();
                            if let Some(texture) = render_ctx.draw_texture(&tile_id, &styled) {
                                tx.send(ThreadResponse::Done { tile_id, texture }).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Drop for ThreadsContext {
    fn drop(&mut self) {
        println!("Stop {} render threads", self.contexts.len());

        for context in &self.contexts {
            if !context.handler.is_finished() {
                println!("thread {}: send Terminate signal", context.no);
                context.cmd_tx.send(ThreadCommand::Terminate).unwrap();
            } else {
                println!("thread {}: already stopped", context.no);
            }
        }

        /* Uncomment when you figure out how to interrupt the render
        while self.contexts.len() != 0 {
            let context = self.contexts.pop();

            match context.unwrap().handler.join() {
                Ok(_) => {}
                Err(_) => {
                    println!("thread.join() failed");
                }
            }
        }
        */
    }
}

impl<'a> RenderContext<'a> {
    fn collect_tile(&self, tile_id: TileId) -> OsmEntities {
        let tile = Tile {
            x: tile_id.x,
            y: tile_id.y,
            zoom: tile_id.zoom,
        };

        self.reader.get_entities_in_tile_with_neighbors(&tile, &None)
    }

    fn collect_styled(&self, tile_id: &TileId, entities: &'a OsmEntities<'a>) -> StyledEntities {
        let tile = Tile {
            x: tile_id.x,
            y: tile_id.y,
            zoom: tile_id.zoom,
        };

        StyledEntities::new(&self.styler, entities, tile.zoom)
    }

    fn draw_tile(&self, tile_id: &TileId, styled: &StyledEntities) -> Option<Vec<u8>> {
        let tile = Tile {
            x: tile_id.x,
            y: tile_id.y,
            zoom: tile_id.zoom,
        };
        let mut current_pixels = TilePixels::new(self.scale);
        Some(
            self.drawer
                .draw(styled, &mut current_pixels, &tile, self.scale as f64, &self.styler)
                .unwrap(),
        )
    }

    fn draw_texture(&self, tile_id: &TileId, styled: &StyledEntities) -> Option<Texture> {
        if let Some(tile_png_bytes) = self.draw_tile(tile_id, styled) {
            match Texture::new(&tile_png_bytes, &self.egui_ctx) {
                Ok(texture) => {
                    self.egui_ctx.request_repaint();
                    Some(texture)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    }
}

pub struct LocalOSMTiles {
    thread_ctx: ThreadsContext,
    displaycache: HashMap<TileId, TextureState>,
    image_waiting: Texture,
    image_collecting: Texture,
    image_styling: Texture,
    image_rendering: Texture,
    image_empty: Texture,
}

impl LocalOSMTiles {
    pub fn new(egui_ctx: Context) -> Option<Self> {
        let styler = match parse_file(Path::new("./localosm/style"), "index.mapcss") {
            Ok(rules) => Styler::new(rules, None),
            Err(err) => {
                log::warn!("MapCSS rules not loaded: {}", err);
                return None;
            }
        };

        let reader = match GeodataReader::load("./localosm/data.bin") {
            Ok(reader) => reader,
            Err(err) => {
                log::warn!("OSM data not loaded: {}", err);
                return None;
            }
        };

        let image_waiting = Texture::new(include_bytes!("../../assets/waiting.png"), &egui_ctx).unwrap();
        let image_collecting = Texture::new(include_bytes!("../../assets/collecting.png"), &egui_ctx).unwrap();
        let image_styling = Texture::new(include_bytes!("../../assets/styling.png"), &egui_ctx).unwrap();
        let image_rendering = Texture::new(include_bytes!("../../assets/rendering.png"), &egui_ctx).unwrap();
        let image_empty = Texture::new(include_bytes!("../../assets/empty.png"), &egui_ctx).unwrap();

        let render_ctx = Arc::new(RenderContext {
            egui_ctx,
            styler,
            drawer: Drawer::new(Path::new("./localosm/style")),
            reader,
            scale: 1,
        });

        let thread_ctx = ThreadsContext::new(render_ctx);

        Some(Self {
            thread_ctx,
            displaycache: Default::default(),
            image_waiting,
            image_styling,
            image_collecting,
            image_rendering,
            image_empty,
        })
    }

    fn is_supported_tile(&self, tile_id: &TileId) -> bool {
        let max_in_line = 1 << tile_id.zoom;

        tile_id.x < max_in_line && tile_id.y < max_in_line
    }
}

impl TilesManager for LocalOSMTiles {
    fn attribution(&self) -> Attribution {
        Attribution {
            text: "OpenStreetMap contributors",
            url: "https://www.openstreetmap.org/copyright",
            logo_light: None,
            logo_dark: None,
        }
    }

    fn tile_size(&self) -> u32 {
        TILE_SIZE
    }

    fn at(&mut self, tile_id: TileId) -> Option<Texture> {
        if !self.is_supported_tile(&tile_id) {
            return None;
        }

        if !self.thread_ctx.is_idle() {
            for message in self.thread_ctx.response_collect() {
                match message {
                    ThreadResponse::Collecting { tile_id } => {
                        *self.displaycache.get_mut(&tile_id).unwrap() = TextureState::Collecting {}
                    }
                    ThreadResponse::Styling { tile_id } => {
                        *self.displaycache.get_mut(&tile_id).unwrap() = TextureState::Styling {}
                    }
                    ThreadResponse::Draw { tile_id } => {
                        *self.displaycache.get_mut(&tile_id).unwrap() = TextureState::Draw {}
                    }
                    ThreadResponse::Done { tile_id, texture } => {
                        *self.displaycache.get_mut(&tile_id).unwrap() = TextureState::Done { texture }
                    }
                    ThreadResponse::Empty { tile_id } => {
                        *self.displaycache.get_mut(&tile_id).unwrap() = TextureState::Empty {}
                    }
                }
            }
        }

        match self.displaycache.entry(tile_id) {
            hash_map::Entry::Occupied(entry) => match entry.get() {
                TextureState::None => Some(self.image_waiting.clone()),
                TextureState::Collecting => Some(self.image_collecting.clone()),
                TextureState::Styling => Some(self.image_styling.clone()),
                TextureState::Draw => Some(self.image_rendering.clone()),
                TextureState::Done { texture } => Some(texture.clone()),
                TextureState::Empty {} => Some(self.image_empty.clone()),
            },
            hash_map::Entry::Vacant(entry) => {
                if !self.thread_ctx.is_busy() {
                    entry.insert(TextureState::None);
                    self.thread_ctx.draw_request(tile_id);
                }
                Some(self.image_waiting.clone())
            }
        }
    }

    fn available_zoom(&self) -> Vec<u8> {
        Vec::from_iter(0..=22)
    }
}
