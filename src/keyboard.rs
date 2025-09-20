use crate::consts::*;
use crate::structs::{Config, PendingKey, RemapAction};
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use log::{debug, info, trace};
use std::sync::{Arc, Mutex};
use std::{collections::HashMap, time::Instant};
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

pub(crate) fn create_virtual_keyboard() -> Result<UInputDevice> {
    let device = uinput::default()
        .map_err(|e| anyhow!("Failed to open /dev/uinput (sudo modprobe uinput): {e}"))?
        .name("Interception Rust Virtual Keyboard")?
        .event(uinput::event::Keyboard::All)?
        .create()?;

    Ok(device)
}

pub(crate) fn process(
    mut device: EvDevDevice,
    virt_keyboard: Arc<Mutex<UInputDevice>>,
    remaps: &HashMap<KeyCode, RemapAction>,
    config: &Config,
) -> Result<()> {
    let mut pending: HashMap<KeyCode, PendingKey> = HashMap::new();

    loop {
        let events = device.fetch_events()?;
        let mut virt_keyboard = virt_keyboard.lock().unwrap();
        for ev in events {
            if ev.event_type() != EventType::KEY {
                continue;
            }

            let state = ev.value();
            let key = KeyCode(ev.code());

            match state {
                PRESS => handle_key_press(&mut virt_keyboard, config, remaps, &mut pending, key)?,
                RELEASE => {
                    handle_key_release(&mut virt_keyboard, config, &mut pending, key)?
                }
                _ => {}
            }
        }
    }
}

pub(crate) fn press(device: &mut UInputDevice, code: KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, code.0 as i32, PRESS)?;
    device.synchronize()?;
    debug!("PRESS: {:?}", code);
    Ok(())
}

pub(crate) fn release(device: &mut UInputDevice, code: KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, code.0 as i32, RELEASE)?;
    device.synchronize()?;
    debug!("RELEASE: {:?}", code);
    Ok(())
}

pub(crate) fn add_pending(
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
    remap: RemapAction,
) {
    pending.entry(key).or_insert(PendingKey {
        remap,
        hold_sent: false,
        time_pressed: Instant::now(),
    });
}

pub(crate) fn remove_pending(
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: &KeyCode,
) -> Option<PendingKey> {
    pending.remove(key)
}

pub(crate) fn send_holds_for_all_pending_keys(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
) -> Result<()> {
    for pending_key in pending.values_mut() {
        if let Some(hold_code) = pending_key.remap.hold
            && !pending_key.hold_sent
        {
            press(virt_keyboard, hold_code, config.no_emit)?;
            pending_key.hold_sent = true;
        }
    }
    Ok(())
}

pub(crate) fn handle_key_press(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    remaps: &HashMap<KeyCode, RemapAction>,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(&remap) = remaps.get(&key) {
        if remap.hold.is_some() {
            add_pending(pending, key, remap);
        } else {
            press(virt_keyboard, remap.tap, config.no_emit)?;
        }
    } else {
        send_holds_for_all_pending_keys(virt_keyboard, config, pending)?;
        press(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}

pub(crate) fn handle_key_release(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(pending_key) = remove_pending(pending, &key) {
        match (pending_key.remap.hold, pending_key.hold_sent) {
            (Some(hold_code), true) => {
                // Release hold remapped
                release(virt_keyboard, hold_code, config.no_emit)?;
            }
            (_, _) => {
                // Tap remapped
                press(virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                release(virt_keyboard, pending_key.remap.tap, config.no_emit)?;
            }
        }
    } else {
        // Release unmapped
        release(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}
