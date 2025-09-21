use crate::config::{Config, KeyboardConfig, Layers, Mappings, RemapAction};
use crate::consts::*;
use anyhow::{Result, anyhow, bail};
use crossbeam_channel::{Receiver, Sender, select, unbounded};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use log::{debug, info};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::time::Instant;
use udev::Enumerator;
use uinput::device::Device as UInputDevice;

type Pending = HashMap<KeyCode, PendingKey>;

pub(crate) struct PendingKey {
    pub remap: RemapAction,
    pub hold_sent: bool,
    pub time_pressed: Instant,
    pub timer_fired: bool,
}

pub(crate) struct Keyboard {
    pub device: EvDevDevice,
    pub config: KeyboardConfig,
}

enum TimerMsg {
    HoldTimeout(KeyCode),
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

                if !config.globals.no_emit {
                    keyboard.grab()?;
                }

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
    let mut virt_keyboard = create_virtual_keyboard(keyboard.device.name().unwrap())?;
    let mut device = keyboard.device;
    let mappings = keyboard.config.mappings;
    let layers = keyboard.config.layers;

    let mut pending: Pending = HashMap::new();
    let mut keys_down: HashSet<KeyCode> = HashSet::new();
    let mut active_layers: HashSet<String> = HashSet::new();

    let (timer_tx, timer_rx): (Sender<TimerMsg>, Receiver<TimerMsg>) = unbounded();

    fn schedule_pending_key_timer(key: KeyCode, duration: Duration, tx: Sender<TimerMsg>) {
        std::thread::spawn(move || {
            std::thread::sleep(duration);
            let _ = tx.send(TimerMsg::HoldTimeout(key));
        });
    }

    loop {
        select! {
            default => {
                match device.fetch_events() {
                    Err(err) => return Err(err.into()),
                    Ok(events) => {
                        for event in events {
                            if event.event_type() != EventType::KEY { continue; }
                            let state = event.value();
                            let key = KeyCode(event.code());
                            let mut is_layer_trigger = false;
                            for (layer_name, layer_def) in &layers {
                                if layer_def.contains_key(&key) {
                                    is_layer_trigger = true;
                                    match state {
                                        PRESS => { active_layers.insert(layer_name.clone()); }
                                        RELEASE => { active_layers.remove(layer_name); }
                                        _ => {}
                                    }
                                    break;
                                }
                            }
                            if is_layer_trigger {
                                match state {
                                    PRESS => { keys_down.insert(key); }
                                    RELEASE => { keys_down.remove(&key); }
                                    _ => {}
                                }
                                continue;
                            }
                            let remapped_keys = resolve_layered_keys(key, &active_layers, &layers);
                            match state {
                                PRESS => {
                                    keys_down.insert(key);
                                    for remapped_key in remapped_keys.clone() {
                                        if let Some(remap) = mappings.get(&remapped_key) {
                                            let schedule = if remap.hrm == Some(true) {
                                                remap.hold.is_some()
                                            } else {
                                                remap.tap.is_some() && remap.hold.is_some()
                                            };
                                            if schedule {
                                                let duration = Duration::from_millis(remap.hrm_term.unwrap_or(config.globals.hrm_term) as u64);
                                                schedule_pending_key_timer(remapped_key, duration, timer_tx.clone());
                                            }
                                        }
                                        handle_key_down(
                                            &mut virt_keyboard,
                                            config,
                                            &mut pending,
                                            remapped_key,
                                            &mappings,
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
            }
            recv(timer_rx) -> msg => {
                if let Ok(TimerMsg::HoldTimeout(key)) = msg
                    && let Some(pending_key) = pending.get_mut(&key)
                        && !pending_key.hold_sent && !pending_key.timer_fired {
                            let remap = &pending_key.remap;
                            if let Some(hold) = &remap.hold {
                                press_keys(&mut virt_keyboard, hold, config.globals.no_emit)?;
                                pending_key.hold_sent = true;
                            }
                            pending_key.timer_fired = true;
                        }
            }
        }
    }
}

fn resolve_layered_keys(
    key: KeyCode,
    active_layers: &HashSet<String>,
    layers: &Layers,
) -> Vec<KeyCode> {
    for layer in active_layers {
        if let Some(layer_map) = layers.get(layer) {
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

fn add_pending(pending: &mut Pending, key: KeyCode, remap: &RemapAction) {
    pending.entry(key).or_insert(PendingKey {
        remap: remap.clone(),
        hold_sent: false,
        time_pressed: Instant::now(),
        timer_fired: false,
    });
}

fn remove_pending(pending: &mut Pending, key: &KeyCode) -> Option<PendingKey> {
    pending.remove(key)
}

fn handle_key_down(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut Pending,
    key: KeyCode,
    mappings: &Mappings,
) -> Result<()> {
    if let Some(remap) = mappings.get(&key) {
        if let Some(ref keys) = remap.tap
            && remap.hold.is_none()
        {
            press_keys(virt_keyboard, keys, config.globals.no_emit)?;
        }
        add_pending(pending, key, remap);
    } else {
        press_key(virt_keyboard, &key, config.globals.no_emit)?;
    }

    Ok(())
}

fn handle_key_up(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut Pending,
    key: KeyCode,
) -> Result<()> {
    if let Some(pending_key) = remove_pending(pending, &key) {
        let remap = pending_key.remap;
        let is_hrm = remap.hrm == Some(true);

        if is_hrm {
            let hrm_term = remap.hrm_term.unwrap_or(config.globals.hrm_term);
            let elapsed = pending_key.time_pressed.elapsed();

            if elapsed < Duration::from_millis(hrm_term as u64) {
                if let Some(tap) = remap.tap {
                    press_keys(virt_keyboard, &tap, config.globals.no_emit)?;
                    release_keys(virt_keyboard, &tap, config.globals.no_emit)?;
                }
            } else if remap.hold.is_some() && pending_key.hold_sent {
                release_keys(virt_keyboard, &remap.hold.unwrap(), config.globals.no_emit)?;
            }
        } else {
            match (remap.tap, remap.hold, pending_key.hold_sent) {
                (_, Some(hold), true) => {
                    release_keys(virt_keyboard, &hold, config.globals.no_emit)?;
                }
                (Some(tap), _, _) => {
                    press_keys(virt_keyboard, &tap, config.globals.no_emit)?;
                    release_keys(virt_keyboard, &tap, config.globals.no_emit)?;
                }
                _ => {}
            }
        }
    } else {
        release_key(virt_keyboard, &key, config.globals.no_emit)?;
    }
    Ok(())
}
