use egui::{Align2, Area, Ui};
use geojson::GeoJson;
use reqwest::header;
use reqwest::Client;
use reqwest::StatusCode;
use std::sync::mpsc;

enum TaskAction {
    Get,
    Publish(GeoJson),
}

enum TaskResult {
    Received(GeoJson),
    PublishOk(String),
    Error(String),
}

struct Task {
    rx: mpsc::Receiver<TaskResult>,
}

impl Task {
    pub fn new(client: Client, action: TaskAction) -> Self {
        let (result_tx, rx) = mpsc::channel();

        #[cfg(not(target_arch = "wasm32"))]
        {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            std::thread::spawn(move || {
                runtime.block_on(async move { Task::dispatch(client, action, result_tx).await })
            });
        }
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move { Task::dispatch(client, action, result_tx).await });

        Self { rx }
    }

    async fn dispatch(client: Client, cmd_req: TaskAction, tx: mpsc::Sender<TaskResult>) {
        match cmd_req {
            TaskAction::Get => Task::get(client, "1".to_string(), &tx).await,
            TaskAction::Publish(geojson) => Task::publish(client, geojson, &tx).await,
        }
    }

    async fn publish(client: Client, geojson: GeoJson, tx: &mpsc::Sender<TaskResult>) {
        let res = match client
            .post("http://127.0.0.1:3000/new")
            .header(header::CONTENT_TYPE, "application/geo+json")
            .body(geojson.to_string())
            .send()
            .await
        {
            Ok(response) => {
                if response.status() == StatusCode::OK {
                    match response.text().await {
                        Ok(identifier) => TaskResult::PublishOk(identifier),
                        Err(err) => TaskResult::Error(format!("Body decoding error: {}", err)),
                    }
                } else {
                    TaskResult::Error(format!("server returns code {}", response.status()))
                }
            }
            Err(err) => TaskResult::Error(err.to_string()),
        };

        let _ = tx.send(res);
    }

    async fn get(client: Client, url: String, tx: &mpsc::Sender<TaskResult>) {
        let res = match client.get(format!("http://127.0.0.1:3000/get/{}", url)).send().await {
            Ok(response) => {
                if response.status() == StatusCode::OK {
                    TaskResult::Received(response.json::<GeoJson>().await.unwrap())
                } else {
                    TaskResult::Error(format!("server returns code {}", response.status()))
                }
            }
            Err(err) => TaskResult::Error(err.to_string()),
        };

        let _ = tx.send(res);
    }
}

pub struct GeoJsonExchange {
    threads_ctx: Vec<Task>,
    statuses: Vec<String>,
    ticker: u16,
    client: Client,
}

impl Default for GeoJsonExchange {
    fn default() -> Self {
        GeoJsonExchange::new()
    }
}

impl GeoJsonExchange {
    pub fn new() -> Self {
        Self {
            threads_ctx: Default::default(),
            statuses: Vec::new(),
            ticker: 0,
            client: Client::new(),
        }
    }

    fn get_responses(&self) -> Vec<TaskResult> {
        let mut result = Vec::new();

        for thread in &self.threads_ctx {
            while let Ok(cmd_res) = thread.rx.try_recv() {
                result.push(cmd_res)
            }
        }
        result
    }

    pub fn update_status(&mut self) {
        if self.ticker == 100 {
            self.ticker = 0;
            self.statuses.pop();
        } else {
            self.ticker += 1;
        }

        for response in self.get_responses() {
            let status = match response {
                TaskResult::Received(_) => "done".to_string(),
                TaskResult::PublishOk(idstr) => format!("published: {}", idstr),
                TaskResult::Error(errstr) => format!("error: {}", errstr),
            };
            self.push_status(status);
        }
    }

    fn push_status(&mut self, status: String) {
        self.statuses.insert(0, status);
        if self.statuses.len() > 10 {
            self.statuses.pop();
        }
    }

    pub fn receive_data(&mut self, id: String) {
        self.threads_ctx.push(Task::new(self.client.clone(), TaskAction::Get));
        self.push_status(format!("receiving {}", id));
    }

    pub fn publish_data(&mut self, json: GeoJson) {
        self.threads_ctx
            .push(Task::new(self.client.clone(), TaskAction::Publish(json)));
        self.push_status("publishing".to_string());
    }

    pub fn show_ui(&mut self, ui: &Ui) {
        Area::new("GeoJson Exchange")
            .anchor(Align2::CENTER_TOP, [0., 30.])
            .interactable(false)
            .show(ui.ctx(), |ui| {
                ui.vertical_centered(|ui| {
                    self.statuses.iter().rev().for_each(|line| {
                        ui.label(line);
                    })
                })
            });
    }
}

impl Drop for GeoJsonExchange {
    fn drop(&mut self) {
        while let Some(_) = self.threads_ctx.pop() {}
    }
}
