use crate::{
    config::{Config, KeyboardConfig},
    layouts::Layout,
};
use anyhow::{Result, anyhow, bail};
use colored::{ColoredString, Colorize};
use crossbeam_channel::{select, unbounded};
use evdev::Device as EvDevDevice;
use evdev::{EventType, InputEvent, KeyCode};
use log::{debug, info};
use std::collections::HashSet;
use udev::Enumerator;
use uinput::Device;
use uinput::device::Device as UInputDevice;

pub(crate) const RELEASE: i32 = 0;
pub(crate) const PRESS: i32 = 1;
pub(crate) const EV_KEY: i32 = 1;

pub(crate) struct Keyboard {
    pub device: EvDevDevice,
    pub config: KeyboardConfig,
}

pub(crate) fn open_keyboard_devices(config: &Config) -> Result<Vec<Keyboard>> {
    debug!("Detecting keyboards");

    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_property("ID_INPUT_KEYBOARD", "1")?;

    let mut keyboards = Vec::new();

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode()
            && let Ok(mut keyboard) = EvDevDevice::open(devnode)
        {
            let name_matches = match keyboard.name() {
                Some(name_value) => config
                    .keyboards
                    .iter()
                    .any(|keyboard| name_value == keyboard.0),
                None => false,
            };

            if name_matches {
                info!("Keyboard Monitored: {:?}", keyboard.name());

                keyboard.grab()?;

                let keyboard_config = keyboard
                    .name()
                    .and_then(|name_value| {
                        config.keyboards.iter().find_map(|(k, v)| {
                            if name_value == k {
                                Some(v.clone())
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or_default();

                keyboards.push(Keyboard {
                    device: keyboard,
                    config: keyboard_config,
                });
            } else {
                debug!("Keyboard Ignored: {:?}", keyboard.name());
            }
        }
    }

    if keyboards.is_empty() {
        bail!("No keyboards found");
    } else {
        Ok(keyboards)
    }
}

pub(crate) fn create_virtual_keyboard(name: &str) -> Result<UInputDevice> {
    let device = uinput::default()
        .map_err(|e| anyhow!("Failed to open /dev/uinput (sudo modprobe uinput): {e}"))?
        .name(format!("{} OxideKeys", name))?
        .event(uinput::event::Keyboard::All)?
        .create()?;
    Ok(device)
}

pub(crate) fn keyboard_processor(keyboard: Keyboard, config: &Config) -> Result<()> {
    let mut virt = create_virtual_keyboard(keyboard.device.name().unwrap())?;
    let mut device = keyboard.device;
    let kb_config = keyboard.config;
    let mut keys_down: HashSet<KeyCode> = HashSet::new();
    let mut active_layer: Option<String> = None;
    let (tx, rx) = unbounded::<InputEvent>();

    let feature_layers_enabled = *config.features.get("layers").unwrap_or(&false);
    let feature_overlaps_enabled = *config.features.get("overlaps").unwrap_or(&false);

    std::thread::spawn(move || {
        loop {
            match device.fetch_events() {
                Err(_) => {
                    break;
                }
                Ok(events) => {
                    for event in events {
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    loop {
        select! {
            recv(rx) -> ev => {
                let event = match ev { Ok(e) => e, Err(_) => break };
                if event.event_type() != EventType::KEY { continue; }
                let state = event.value();
                if state > PRESS { continue; }
                let key_raw = KeyCode(event.code());
                let key_layout = kb_config.layout.resolve(&key_raw);

                let mut key_handled = false;

                if !key_handled && feature_layers_enabled {
                    key_handled = feature_layers(&mut virt, &kb_config, &key_layout, state, &mut keys_down, &mut active_layer)?;
                }

                if !key_handled && feature_overlaps_enabled {
                    key_handled = feature_overlaps(&mut virt, &kb_config, &key_layout, state, &mut keys_down, &mut active_layer)?;
                }

                if !key_handled {
                    send_key(&mut virt, &kb_config.layout, &key_layout, state)?;
                }
            }
        }
    }

    Ok(())
}

fn feature_overlaps(
    _virt: &mut Device,
    _kb_config: &KeyboardConfig,
    _key_layout: &KeyCode,
    _state: i32,
    _keys_down: &mut HashSet<KeyCode>,
    _active_layer: &mut Option<String>,
) -> Result<bool> {
    Ok(false)
}

fn feature_layers(
    virt: &mut Device,
    kb_config: &KeyboardConfig,
    key_layout: &KeyCode,
    state: i32,
    keys_down: &mut HashSet<KeyCode>,
    active_layer: &mut Option<String>,
) -> Result<bool> {
    for (layer_name, layer_def) in &kb_config.layers {
        if layer_def.contains_key(key_layout) {
            match state {
                PRESS => {
                    keys_down.insert(*key_layout);
                    *active_layer = Some(layer_name.to_owned());
                }
                RELEASE => {
                    keys_down.remove(key_layout);
                    *active_layer = None;
                }
                _ => {}
            }

            log_layer(layer_name, state);

            return Ok(true);
        }
    }

    if let Some(layer_name) = active_layer
        && let Some(layer_map) = kb_config.layers.get(layer_name)
    {
        for mapping in layer_map.values() {
            if let Some(remapped) = mapping.get(key_layout) {
                send_keys(virt, &kb_config.layout, remapped, state)?;
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn send_key(virt: &mut Device, layout: &Layout, key: &KeyCode, state: i32) -> Result<()> {
    let resolved_key = layout.resolve_reverse(key);
    virt.write(EV_KEY, resolved_key.0 as i32, state)?;
    virt.synchronize()?;
    log_key(key, state);
    Ok(())
}

fn send_keys(virt: &mut Device, layout: &Layout, keys: &Vec<KeyCode>, state: i32) -> Result<()> {
    for key in keys {
        let resolved_key = layout.resolve_reverse(key);
        virt.write(EV_KEY, resolved_key.0 as i32, state)?;
    }
    virt.synchronize()?;
    log_keys(keys, state);
    Ok(())
}

fn log_keys(keys: &[KeyCode], state: i32) {
    let key_str = keys
        .iter()
        .map(|k| format!("{:?}", k).chars().skip(4).collect::<String>())
        .collect::<Vec<_>>()
        .join(", ");

    debug!(
        "{} {}: {}",
        state_arrow(state),
        "KEYS".yellow(),
        key_str.bright_blue(),
    );
}

fn log_key(key: &KeyCode, state: i32) {
    debug!(
        "{} {}: {}",
        state_arrow(state),
        "KEY".yellow(),
        &format!("{:?}", key)[4..].bright_blue(),
    );
}

fn log_layer(layer: &str, state: i32) {
    debug!(
        "{} {}: {}",
        state_arrow(state),
        "LAYER".purple(),
        layer.bright_blue(),
    );
}

fn state_arrow(state: i32) -> ColoredString {
    match state {
        PRESS => "↓".green().bold(),
        _ => "↑".red().bold(),
    }
}

#[allow(dead_code)]
fn is_modifier(key: &KeyCode) -> bool {
    matches!(
        *key,
        KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
    )
}
