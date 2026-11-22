use serde_json::{json, Map, Value};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Default)]
pub struct Activity {
    pub state: Option<String>,
    pub details: Option<String>,
    pub start: Option<i64>,
    pub end: Option<i64>,
    pub elapsed: bool,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    pub buttons: Vec<(String, String)>,
}

enum Command {
    Set(Activity),
    Disconnect,
}

pub struct Discord {
    tx: Sender<Command>,
}

impl Discord {
    pub fn start(client_id: String, pid: u32) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || run(client_id, pid, rx));
        Self { tx }
    }

    pub fn set(&self, activity: Activity) {
        let _ = self.tx.send(Command::Set(activity));
    }

    pub fn disconnect(&self) {
        let _ = self.tx.send(Command::Disconnect);
    }
}

fn run(client_id: String, pid: u32, rx: Receiver<Command>) {
    let mut connection: Option<Connection> = None;
    let mut pending: Option<Activity> = None;

    loop {
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(Command::Disconnect) => {
                connection = None;
                pending = None;
            }
            Ok(Command::Set(activity)) => pending = Some(activity),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let Some(activity) = pending.clone() else {
            continue;
        };

        if connection.is_none() {
            connection = Connection::open(&client_id);
        }

        if let Some(current) = connection.as_mut() {
            match current.set_activity(pid, &activity) {
                Ok(()) => pending = None,
                Err(()) => connection = None,
            }
        }
    }
}

struct Connection {
    file: File,
}

impl Connection {
    fn open(client_id: &str) -> Option<Self> {
        for pipe in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{pipe}");
            let Ok(file) = OpenOptions::new().read(true).write(true).open(path) else {
                continue;
            };
            let mut connection = Self { file };
            let handshake = json!({ "v": 1, "client_id": client_id });
            if connection.send_frame(0, &handshake).is_ok() {
                return Some(connection);
            }
        }
        None
    }

    fn set_activity(&mut self, pid: u32, activity: &Activity) -> Result<(), ()> {
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "nonce": nonce(),
            "args": {
                "pid": pid,
                "activity": to_activity(activity)
            }
        });
        self.send_frame(1, &payload)
    }

    fn send_frame(&mut self, opcode: u32, payload: &Value) -> Result<(), ()> {
        let text = payload.to_string();
        let mut packet = Vec::with_capacity(8 + text.len());
        packet.extend_from_slice(&opcode.to_le_bytes());
        packet.extend_from_slice(&(text.len() as u32).to_le_bytes());
        packet.extend_from_slice(text.as_bytes());
        self.file.write_all(&packet).map_err(|_| ())
    }
}

fn to_activity(activity: &Activity) -> Value {
    let mut result = Map::new();
    if let Some(details) = &activity.details {
        result.insert("details".into(), json!(details));
    }
    if let Some(state) = &activity.state {
        result.insert("state".into(), json!(state));
    }

    let start = if activity.elapsed {
        Some(now_ms())
    } else {
        activity.start
    };
    let mut timestamps = Map::new();
    if let Some(start) = start {
        timestamps.insert("start".into(), json!(start));
    }
    if let Some(end) = activity.end {
        timestamps.insert("end".into(), json!(end));
    }
    if !timestamps.is_empty() {
        result.insert("timestamps".into(), Value::Object(timestamps));
    }

    let mut assets = Map::new();
    if let Some(image) = &activity.large_image {
        assets.insert("large_image".into(), json!(image));
    }
    if let Some(text) = &activity.large_text {
        assets.insert("large_text".into(), json!(text));
    }
    if let Some(image) = &activity.small_image {
        assets.insert("small_image".into(), json!(image));
    }
    if let Some(text) = &activity.small_text {
        assets.insert("small_text".into(), json!(text));
    }
    if !assets.is_empty() {
        result.insert("assets".into(), Value::Object(assets));
    }

    let buttons: Vec<Value> = activity
        .buttons
        .iter()
        .filter(|(label, url)| {
            !label.is_empty() && (url.starts_with("https://") || url.starts_with("http://"))
        })
        .take(2)
        .map(|(label, url)| json!({ "label": label, "url": url }))
        .collect();
    if !buttons.is_empty() {
        result.insert("buttons".into(), json!(buttons));
    }

    result.insert("type".into(), json!(3));
    result.insert("instance".into(), json!(true));
    Value::Object(result)
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

fn nonce() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}
