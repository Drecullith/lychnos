use std::{
    env,
    fs::OpenOptions,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

use lychnos_core::perception::{PerceptionControl, PerceptionEvent};
use lychnos_core::voice::AudioInputDevice;

pub struct PerceptionWorker {
    receiver: Receiver<PerceptionEvent>,
    stdin: ChildStdin,
    child: Child,
}

impl PerceptionWorker {
    pub fn start(device: &AudioInputDevice) -> Result<Self, String> {
        let binary = discover_binary()
            .ok_or_else(|| "Voice V2 perception binary is not installed or built".to_string())?;

        let log_path = perception_log_path();
        if let Some(parent) = log_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create perception log directory: {error}"))?;
        }
        let stderr = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|error| format!("failed to open {}: {error}", log_path.display()))?;

        let mut child = Command::new(&binary)
            .env("LYCHNOS_AUDIO_SOURCE", device.id.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| {
                format!(
                    "failed to start Voice V2 perception {}: {error}",
                    binary.display()
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Voice V2 perception stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Voice V2 perception stdout unavailable".to_string())?;
        let (sender, receiver) = mpsc::channel();

        thread::Builder::new()
            .name("lychnos-perception-events".into())
            .spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else {
                        break;
                    };
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    match serde_json::from_str::<PerceptionEvent>(trimmed) {
                        Ok(event) => {
                            if sender.send(event).is_err() {
                                break;
                            }
                        }
                        Err(error) => {
                            eprintln!(
                                "Ignoring invalid Voice V2 perception event {trimmed:?}: {error}"
                            );
                        }
                    }
                }
            })
            .map_err(|error| format!("failed to start perception event reader: {error}"))?;

        Ok(Self {
            receiver,
            stdin,
            child,
        })
    }

    pub fn try_recv(&self) -> Option<PerceptionEvent> {
        self.receiver.try_recv().ok()
    }

    pub fn send(&mut self, control: PerceptionControl) -> Result<(), String> {
        let json = serde_json::to_string(&control)
            .map_err(|error| format!("failed to encode perception control: {error}"))?;
        writeln!(self.stdin, "{json}")
            .and_then(|_| self.stdin.flush())
            .map_err(|error| format!("failed to send perception control: {error}"))
    }

    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }
}

impl Drop for PerceptionWorker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn discover_binary() -> Option<PathBuf> {
    if let Some(path) = env::var_os("LYCHNOS_PERCEPTION_BIN").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }

    if let Ok(current) = env::current_exe()
        && let Some(parent) = current.parent()
    {
        let sibling = parent.join("lychnos-perception-omarchy");
        if sibling.is_file() {
            return Some(sibling);
        }
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    let installed = home.join(".local/lib/lychnos/lychnos-perception-omarchy");
    installed.is_file().then_some(installed)
}

fn perception_log_path() -> PathBuf {
    if let Some(state_home) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("lychnos/perception-v2.log");
    }

    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    home.join(".local/state/lychnos/perception-v2.log")
}
