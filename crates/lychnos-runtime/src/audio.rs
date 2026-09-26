use std::{
    env, fs,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use lychnos_core::voice::{AudioInputDevice, AudioInputDeviceId, VoiceCaptureId};
use serde_json::Value;

pub struct ActiveCapture {
    pub capture_id: VoiceCaptureId,
    pub device: AudioInputDevice,
    child: Child,
    path: PathBuf,
    started_at: Instant,
}

impl ActiveCapture {
    pub fn elapsed(&self) -> Duration {
        self.started_at.elapsed()
    }
}

pub struct CompletedCapture {
    pub capture_id: VoiceCaptureId,
    pub device: AudioInputDevice,
    pub path: PathBuf,
    pub bytes: u64,
    pub duration_ms: u64,
}

pub fn start_push_to_talk(
    capture_id: VoiceCaptureId,
    requested_device: Option<&AudioInputDeviceId>,
) -> Result<ActiveCapture, String> {
    let device = prepare_input_device(requested_device)?;

    let directory = audio_capture_directory();
    fs::create_dir_all(&directory)
        .map_err(|error| format!("failed to create {}: {error}", directory.display()))?;

    let path = directory.join(format!("ptt-{}-{}.wav", std::process::id(), unique_nanos()));

    let child = Command::new("pw-record")
        .args([
            "--target",
            device.id.as_str(),
            "--rate",
            "16000",
            "--channels",
            "1",
            "--format",
            "s16",
            "--container",
            "wav",
        ])
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to start pw-record: {error}"))?;

    Ok(ActiveCapture {
        capture_id,
        device,
        child,
        path,
        started_at: Instant::now(),
    })
}

pub fn prepare_default_input() -> Result<AudioInputDevice, String> {
    prepare_input_device(None)
}

fn prepare_input_device(
    requested_device: Option<&AudioInputDeviceId>,
) -> Result<AudioInputDevice, String> {
    let devices = discover_pipewire_inputs()?;
    let device = match requested_device {
        Some(requested) => devices
            .into_iter()
            .find(|device| &device.id == requested)
            .ok_or_else(|| format!("requested input not found: {}", requested.as_str()))?,
        None => devices
            .iter()
            .find(|device| device.is_default)
            .cloned()
            .or_else(|| devices.into_iter().next())
            .ok_or_else(|| "no microphone input is available".to_string())?,
    };

    apply_configured_input_settings(&device)?;
    Ok(device)
}

fn apply_configured_input_settings(device: &AudioInputDevice) -> Result<(), String> {
    if let Ok(port) = env::var("LYCHNOS_AUDIO_SOURCE_PORT") {
        let port = port.trim();
        if !port.is_empty() {
            let status = Command::new("pactl")
                .args(["set-source-port", device.id.as_str(), port])
                .status()
                .map_err(|error| format!("failed to set configured microphone port: {error}"))?;
            if !status.success() {
                return Err(format!(
                    "could not select configured microphone port {port} for {}",
                    device.id.as_str()
                ));
            }
        }
    }

    if let Ok(raw_percent) = env::var("LYCHNOS_AUDIO_INPUT_VOLUME_PERCENT") {
        let percent = parse_volume_percent(&raw_percent)?;
        let volume = format!("{percent}%");
        let status = Command::new("pactl")
            .args(["set-source-volume", device.id.as_str(), &volume])
            .status()
            .map_err(|error| format!("failed to set configured microphone level: {error}"))?;
        if !status.success() {
            return Err(format!(
                "could not set configured microphone level {volume} for {}",
                device.id.as_str()
            ));
        }
    }

    Ok(())
}

fn parse_volume_percent(raw: &str) -> Result<u16, String> {
    let percent = raw
        .trim()
        .parse::<u16>()
        .map_err(|error| format!("invalid microphone level {raw:?}: {error}"))?;

    if percent > 150 {
        return Err(format!(
            "microphone level {percent}% is outside the supported 0-150% range"
        ));
    }

    Ok(percent)
}

pub fn stop_push_to_talk(mut capture: ActiveCapture) -> Result<CompletedCapture, String> {
    let duration_ms = u64::try_from(capture.elapsed().as_millis()).unwrap_or(u64::MAX);
    let pid = capture.child.id().to_string();
    let signal = Command::new("kill")
        .args(["-INT", &pid])
        .status()
        .map_err(|error| format!("failed to signal pw-record: {error}"))?;

    if !signal.success() {
        return Err(format!("failed to stop pw-record process {pid}"));
    }

    let status = wait_for_exit(&mut capture.child, Duration::from_millis(750))?
        .or_else(|| {
            let _ = Command::new("kill").args(["-TERM", &pid]).status();
            wait_for_exit(&mut capture.child, Duration::from_millis(500))
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| {
            let _ = capture.child.kill();
            capture
                .child
                .wait()
                .unwrap_or_else(|_| synthetic_failure_status())
        });

    let bytes = fs::metadata(&capture.path)
        .map_err(|error| format!("failed to inspect {}: {error}", capture.path.display()))?
        .len();

    if !status.success() && bytes <= 44 {
        return Err(format!(
            "pw-record exited with {status} and produced no usable audio"
        ));
    }

    Ok(CompletedCapture {
        capture_id: capture.capture_id,
        device: capture.device,
        path: capture.path,
        bytes,
        duration_ms,
    })
}

fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<Option<ExitStatus>, String> {
    let deadline = Instant::now() + timeout;

    loop {
        match child
            .try_wait()
            .map_err(|error| format!("failed to poll pw-record: {error}"))?
        {
            Some(status) => return Ok(Some(status)),
            None if Instant::now() >= deadline => return Ok(None),
            None => thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[cfg(unix)]
fn synthetic_failure_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(1 << 8)
}

fn audio_capture_directory() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("lychnos/audio");
    }

    std::env::temp_dir().join("lychnos/audio")
}

fn unique_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after Unix epoch")
        .as_nanos()
}

pub fn discover_pipewire_inputs() -> Result<Vec<AudioInputDevice>, String> {
    let output = Command::new("pw-dump")
        .output()
        .map_err(|error| format!("failed to run pw-dump: {error}"))?;

    if !output.status.success() {
        return Err(format!(
            "pw-dump exited with {}",
            output
                .status
                .code()
                .map_or_else(|| "signal".to_string(), |code| code.to_string())
        ));
    }

    let objects: Vec<Value> = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid pw-dump JSON: {error}"))?;
    let default_name = default_source_name();

    let mut devices = Vec::new();

    for object in objects {
        let props = object
            .get("info")
            .and_then(|info| info.get("props"))
            .and_then(Value::as_object);

        let Some(props) = props else {
            continue;
        };

        if props.get("media.class").and_then(Value::as_str) != Some("Audio/Source") {
            continue;
        }

        let Some(node_name) = props.get("node.name").and_then(Value::as_str) else {
            continue;
        };

        if node_name.ends_with(".monitor") {
            continue;
        }

        let display_name = props
            .get("node.description")
            .and_then(Value::as_str)
            .or_else(|| props.get("node.nick").and_then(Value::as_str))
            .unwrap_or(node_name);

        let mut device =
            AudioInputDevice::new(AudioInputDeviceId::new(node_name), display_name, "pipewire")
                .with_default(default_name.as_deref() == Some(node_name));

        if let Some(channels) = props
            .get("audio.channels")
            .and_then(Value::as_u64)
            .and_then(|channels| u32::try_from(channels).ok())
        {
            device = device.with_channel_count(channels);
        }

        devices.push(device);
    }

    devices.sort_by(|left, right| {
        right
            .is_default
            .cmp(&left.is_default)
            .then_with(|| left.display_name.cmp(&right.display_name))
    });

    Ok(devices)
}

fn default_source_name() -> Option<String> {
    let output = Command::new("wpctl")
        .args(["inspect", "@DEFAULT_AUDIO_SOURCE@"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout.lines().find_map(|line| {
        let (_, value) = line.trim().split_once("node.name =")?;
        Some(value.trim().trim_matches('"').to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::parse_volume_percent;

    #[test]
    fn configured_volume_percent_is_bounded() {
        assert_eq!(parse_volume_percent("30").expect("valid level"), 30);
        assert!(parse_volume_percent("151").is_err());
        assert!(parse_volume_percent("loud").is_err());
    }

    #[test]
    fn default_source_parser_tolerates_wpctl_prefix_marker() {
        let line = "* node.name = \"alsa_input.example\"";
        let (_, value) = line.trim().split_once("node.name =").expect("node name");
        assert_eq!(value.trim().trim_matches('"'), "alsa_input.example");
    }
}
