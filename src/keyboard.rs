use crate::config::{Config, KeyboardConfig};
use crate::layouts::Layout;
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
    let mut holds_triggered: HashSet<KeyCode> = HashSet::new();
    let mut active_layer: Option<String> = None;
    let (tx, rx) = unbounded::<InputEvent>();

    let layout = crate::layouts::get(&kb_config.layout);

    let feature_layers_enabled = *config.features.get("layers").unwrap_or(&false);
    let feature_dual_function_enabled = *config.features.get("dual_function").unwrap_or(&false);

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
                let key_layout = layout.to(&key_raw);

                let mut key_handled = false;

                if !key_handled && feature_layers_enabled {
                    key_handled = feature_layers(&mut virt, &kb_config, &layout, &key_layout, state, &mut keys_down, &mut active_layer)?;
                }

                if !key_handled && feature_dual_function_enabled {
                    key_handled = feature_dual_function(&mut virt, &kb_config, &layout, &key_layout, state, &mut keys_down, &mut holds_triggered)?;
                }

                if !key_handled {
                    send_key(&mut virt, &layout, &key_layout, state)?;
                }
            }
        }
    }

    Ok(())
}

/// Dual Function - Tap-Hold
/// - If you press and release a key without overlapping another, Tap fires.
/// - If you press the key and while it's held another key overlaps, Hold fires.
fn feature_dual_function(
    virt: &mut Device,
    kb_config: &KeyboardConfig,
    layout: &Box<dyn Layout>,
    key: &KeyCode,
    state: i32,
    keys_down: &mut HashSet<KeyCode>,
    holds_triggered: &mut HashSet<KeyCode>,
) -> Result<bool> {
    if let Some(remap) = kb_config.mappings.get(key) {
        match state {
            PRESS => {
                keys_down.insert(*key);

                let overlap_now = keys_down.len() > 1;
                if overlap_now {
                    holds_triggered.insert(*key);

                    if let Some(hold_keys) = &remap.hold {
                        send_keys(virt, layout, hold_keys, PRESS)?;
                    }
                }

                return Ok(true);
            }
            RELEASE => {
                let was_hold = holds_triggered.remove(key);
                keys_down.remove(key);

                if was_hold {
                    if let Some(hold_keys) = &remap.hold {
                        send_keys(virt, layout, hold_keys, RELEASE)?;
                    }
                } else {
                    if let Some(tap_keys) = &remap.tap {
                        send_keys(virt, layout, tap_keys, PRESS)?;
                        send_keys(virt, layout, tap_keys, RELEASE)?;
                    }
                }

                return Ok(true);
            }
            _ => {}
        }

        return Ok(true);
    }

    if state == PRESS && !keys_down.is_empty() && !keys_down.contains(key) {
        for origin in keys_down.iter() {
            if !holds_triggered.contains(origin) {
                if let Some(remap) = kb_config.mappings.get(origin) {
                    if let Some(hold_keys) = &remap.hold {
                        send_keys(virt, layout, hold_keys, PRESS)?;
                    }

                    holds_triggered.insert(*origin);
                }
            }
        }

        return Ok(false);
    }

    Ok(false)
}

fn feature_layers(
    virt: &mut Device,
    kb_config: &KeyboardConfig,
    layout: &Box<dyn Layout>,
    key: &KeyCode,
    state: i32,
    keys_down: &mut HashSet<KeyCode>,
    active_layer: &mut Option<String>,
) -> Result<bool> {
    for (layer_name, layer_def) in &kb_config.layers {
        if layer_def.contains_key(key) {
            match state {
                PRESS => {
                    keys_down.insert(*key);
                    *active_layer = Some(layer_name.to_owned());
                }
                RELEASE => {
                    keys_down.remove(key);
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
            if let Some(remapped) = mapping.get(key) {
                send_keys(virt, layout, remapped, state)?;
                return Ok(true);
            }
        }
    }

    Ok(false)
}

fn send_key(virt: &mut Device, layout: &Box<dyn Layout>, key: &KeyCode, state: i32) -> Result<()> {
    let resolved_key = layout.from(key);
    virt.write(EV_KEY, resolved_key.0 as i32, state)?;
    virt.synchronize()?;
    log_key(key, state);
    Ok(())
}

fn send_keys(
    virt: &mut Device,
    layout: &Box<dyn Layout>,
    keys: &Vec<KeyCode>,
    state: i32,
) -> Result<()> {
    for key in keys {
        let resolved_key = layout.from(key);
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
