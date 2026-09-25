use std::process::Command;

use lychnos_core::voice::{AudioInputDevice, AudioInputDeviceId};
use serde_json::Value;

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
    #[test]
    fn default_source_parser_tolerates_wpctl_prefix_marker() {
        let line = "* node.name = \"alsa_input.example\"";
        let (_, value) = line.trim().split_once("node.name =").expect("node name");
        assert_eq!(value.trim().trim_matches('"'), "alsa_input.example");
    }
}
