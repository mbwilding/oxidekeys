use crate::consts::*;
use crate::structs::{Config, PendingKey, RemapAction};
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use log::{debug, info};
use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::time::Duration;
use std::time::Instant;
use udev::Enumerator;
use uinput::device::Device as UInputDevice;

pub(crate) fn open_keyboard_devices(
    config: &Config,
) -> Result<Vec<(EvDevDevice, HashMap<KeyCode, RemapAction>)>> {
    debug!("Detecting keyboards");

    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_property("ID_INPUT_KEYBOARD", "1")?;

    let mut devices = Vec::new();

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode()
            && let Ok(mut dev) = EvDevDevice::open(devnode)
        {
            let name_matches = match dev.name() {
                Some(name_value) => config
                    .keyboards
                    .iter()
                    .any(|keyboard| name_value == keyboard.0),
                None => false,
            };
            if name_matches {
                info!("Keyboard Monitored: {:?}", dev.name());

                dev.set_nonblocking(true)?;

                if !config.no_emit {
                    dev.grab()?;
                }

                let keyboard_value = dev
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

                devices.push((dev, keyboard_value));
            } else {
                debug!("Keyboard Ignored: {:?}", dev.name());
            }
        }
    }

    if devices.is_empty() {
        bail!("No keyboards found");
    } else {
        Ok(devices)
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

pub(crate) fn process(
    mut device: EvDevDevice,
    mut virt_keyboard: UInputDevice,
    remaps: &HashMap<KeyCode, RemapAction>,
    config: &Config,
) -> Result<()> {
    let mut pending: HashMap<KeyCode, PendingKey> = HashMap::new();
    let mut keys_down: HashSet<KeyCode> = HashSet::new();
    let mut active_layers: HashSet<String> = HashSet::new();
    let mut flush_keys = Vec::new();

    loop {
        let events = match device.fetch_events() {
            Ok(ev) => ev,
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                send_holds_for_all_pending_keys(&mut virt_keyboard, config, &mut pending, &keys_down)?;
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(err) => return Err(err.into()),
        };

        for ev in events {
            if ev.event_type() != EventType::KEY {
                continue;
            }

            let state = ev.value();
            let key = KeyCode(ev.code());

            let mut is_layer_trigger = false;
            for (layer_name, layer_def) in &config.layers {
                if layer_def.contains_key(&key) {
                    is_layer_trigger = true;
                    match state {
                        PRESS => {
                            active_layers.insert(layer_name.clone());
                        }
                        RELEASE => {
                            active_layers.remove(layer_name);
                        }
                        _ => {}
                    }
                    break;
                }
            }
            if is_layer_trigger {
                match state {
                    PRESS => {
                        keys_down.insert(key);
                    }
                    RELEASE => {
                        keys_down.remove(&key);
                    }
                    _ => {}
                }
                continue;
            }

            let remapped_keys = resolve_layered_keys(key, &active_layers, config);

            match state {
                PRESS => {
                    flush_keys.clear();

                    for (pending_keycode, pending_key) in pending.iter_mut() {
                        let remap = &pending_key.remap;
                        if remap.hrm == Some(true)
                            && !pending_key.hold_sent
                            && !is_modifier(key)
                            && key != *pending_keycode
                        {
                            let hrm_term = remap.hrm_term.unwrap_or(config.hrm_term);
                            let elapsed = pending_key.time_pressed.elapsed();
                            if elapsed >= Duration::from_millis(hrm_term as u64) {
                                if let Some(hold) = &remap.hold {
                                    press_keys(&mut virt_keyboard, hold, config.no_emit)?;
                                    pending_key.hold_sent = true;
                                }
                            } else {
                                flush_keys.push(*pending_keycode);
                            }
                        }
                    }

                    for flush_key in &flush_keys {
                        if let Some(pending_key) = remove_pending(&mut pending, flush_key) {
                            let remap = pending_key.remap;
                            if let Some(tap) = remap.tap {
                                press_keys(&mut virt_keyboard, &tap, config.no_emit)?;
                                release_keys(&mut virt_keyboard, &tap, config.no_emit)?;
                            }
                        }
                    }

                    keys_down.insert(key);
                    for remapped_key in remapped_keys {
                        handle_key_down(
                            &mut virt_keyboard,
                            config,
                            &mut pending,
                            remapped_key,
                            remaps,
                        )?;
                    }
                }
                RELEASE => {
                    keys_down.remove(&key);
                    for remapped_key in remapped_keys {
                        handle_key_up(&mut virt_keyboard, config, &mut pending, remapped_key)?;
                    }
                }
                _ => {}
            }
        }
    }
}

fn is_modifier(key: KeyCode) -> bool {
    matches!(
        KeyCode::new(key.0),
        KeyCode::KEY_LEFTSHIFT
            | KeyCode::KEY_RIGHTSHIFT
            | KeyCode::KEY_LEFTCTRL
            | KeyCode::KEY_RIGHTCTRL
            | KeyCode::KEY_LEFTALT
            | KeyCode::KEY_RIGHTALT
            | KeyCode::KEY_LEFTMETA
            | KeyCode::KEY_RIGHTMETA
            | KeyCode::KEY_CAPSLOCK
    )
}

fn resolve_layered_keys(
    key: KeyCode,
    active_layers: &HashSet<String>,
    config: &Config,
) -> Vec<KeyCode> {
    for layer in active_layers {
        if let Some(layer_map) = config.layers.get(layer) {
            for mapping in layer_map.values() {
                if let Some(remapped) = mapping.get(&key) {
                    return remapped.clone();
                }
            }
        }
    }

    vec![key]
}

fn press_key(device: &mut UInputDevice, key: &KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, key.0 as i32, PRESS)?;
    device.synchronize()?;
    debug!("PRESS: {:?}", key);
    Ok(())
}

fn press_keys(device: &mut UInputDevice, keys: &[KeyCode], no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    for key in keys {
        device.write(EV_KEY, key.0 as i32, PRESS)?;
    }
    device.synchronize()?;
    debug!("PRESS: {:?}", keys);
    Ok(())
}

fn release_key(device: &mut UInputDevice, key: &KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, key.0 as i32, RELEASE)?;
    device.synchronize()?;
    debug!("RELEASE: {:?}", key);
    Ok(())
}

fn release_keys(device: &mut UInputDevice, keys: &[KeyCode], no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    for key in keys {
        device.write(EV_KEY, key.0 as i32, RELEASE)?;
    }
    device.synchronize()?;
    debug!("RELEASE: {:?}", keys);
    Ok(())
}

fn add_pending(pending: &mut HashMap<KeyCode, PendingKey>, key: KeyCode, remap: &RemapAction) {
    pending.entry(key).or_insert(PendingKey {
        remap: remap.clone(),
        hold_sent: false,
        time_pressed: Instant::now(),
    });
}

fn remove_pending(pending: &mut HashMap<KeyCode, PendingKey>, key: &KeyCode) -> Option<PendingKey> {
    pending.remove(key)
}

fn send_holds_for_all_pending_keys(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    keys_down: &HashSet<KeyCode>,
) -> Result<()> {
    for (pending_keycode, pending_key) in pending.iter_mut() {
        let remap = &pending_key.remap;

        if remap.hrm == Some(true) {
            let hrm_term = remap.hrm_term.unwrap_or(config.hrm_term);
            let elapsed = pending_key.time_pressed.elapsed();
            if let Some(hold) = &remap.hold
                && !pending_key.hold_sent
                && elapsed >= Duration::from_millis(hrm_term as u64)
            {
                press_keys(virt_keyboard, hold, config.no_emit)?;
                pending_key.hold_sent = true;
            }
        } else if let Some(hold) = &remap.hold
            && !pending_key.hold_sent
        {
            let is_overlay = remap.tap.is_some() && remap.hold.is_some();
            // For overlays: only trigger "hold" if another non-modifier key is being held.
            if is_overlay {
                let other_non_modifiers = keys_down.iter()
                    .filter(|&&k| k != *pending_keycode && !is_modifier(k))
                    .count();
                if other_non_modifiers > 0 {
                    press_keys(virt_keyboard, hold, config.no_emit)?;
                    pending_key.hold_sent = true;
                }
            } else {
                // If not an overlay, send hold immediately.
                press_keys(virt_keyboard, hold, config.no_emit)?;
                pending_key.hold_sent = true;
            }
        }
    }
    Ok(())
}

fn handle_key_down(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
    remaps: &HashMap<KeyCode, RemapAction>,
) -> Result<()> {
    if let Some(remap) = remaps.get(&key) {
        if let Some(ref keys) = remap.tap
            && remap.hold.is_none()
        {
            press_keys(virt_keyboard, keys, config.no_emit)?;
        }
        add_pending(pending, key, remap);
    } else {
        press_key(virt_keyboard, &key, config.no_emit)?;
    }

    Ok(())
}

fn handle_key_up(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(pending_key) = remove_pending(pending, &key) {
        let remap = pending_key.remap;
        let is_hrm = remap.hrm == Some(true);

        if is_hrm {
            let hrm_term = remap.hrm_term.unwrap_or(config.hrm_term);
            let elapsed = pending_key.time_pressed.elapsed();

            if elapsed < Duration::from_millis(hrm_term as u64) {
                if let Some(tap) = remap.tap {
                    press_keys(virt_keyboard, &tap, config.no_emit)?;
                    release_keys(virt_keyboard, &tap, config.no_emit)?;
                }
            } else if remap.hold.is_some() && pending_key.hold_sent {
                release_keys(virt_keyboard, &remap.hold.unwrap(), config.no_emit)?;
            }
        } else {
            match (remap.tap, remap.hold, pending_key.hold_sent) {
                (_, Some(hold), true) => {
                    release_keys(virt_keyboard, &hold, config.no_emit)?;
                }
                (Some(tap), _, _) => {
                    press_keys(virt_keyboard, &tap, config.no_emit)?;
                    release_keys(virt_keyboard, &tap, config.no_emit)?;
                }
                _ => {}
            }
        }
    } else {
        release_key(virt_keyboard, &key, config.no_emit)?;
    }
    Ok(())
}
