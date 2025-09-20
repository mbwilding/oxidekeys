mod consts;
mod structs;

use crate::consts::*;
use crate::structs::*;
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use std::collections::HashMap;
use udev::Enumerator;
use uinput::device::Device as UInputDevice;

fn open_keyboard_devices(filter: &[String], no_emit: bool) -> Result<EvDevDevice> {
    println!("Detecting keyboards");

    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_property("ID_INPUT_KEYBOARD", "1")?;

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode()
            && let Ok(mut dev) = EvDevDevice::open(devnode)
        {
            let name_matches = match dev.name() {
                Some(name_value) => filter.iter().any(|item| name_value == item),
                None => false,
            };
            if name_matches {
                println!("Keyboard Monitored: {:?}", dev.name());
                if !no_emit {
                    dev.grab()?;
                }
                return Ok(dev);
            } else {
                println!("Keyboard Ignored: {:?}", dev.name());
            }
        }
    }

    bail!("No keyboard found");
}

fn create_virtual_keyboard() -> Result<UInputDevice> {
    let device = uinput::default()
        .map_err(|e| anyhow!("Failed to open /dev/uinput: {e}"))?
        .name("Interception Rust Virtual Keyboard")?
        .event(uinput::event::Keyboard::All)?
        .create()?;

    Ok(device)
}

fn press(device: &mut UInputDevice, code: KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, code.0 as i32, PRESS)?;
    device.synchronize()?;
    println!("PRESS: {:?}", code);
    Ok(())
}

fn release(device: &mut UInputDevice, code: KeyCode, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, code.0 as i32, RELEASE)?;
    device.synchronize()?;
    println!("RELEASE: {:?}", code);
    Ok(())
}

fn main() -> Result<()> {
    let config_content = std::fs::read_to_string("config.yaml")?;
    let config: Config = serde_yaml::from_str(&config_content)?;
    println!("Config: {:#?}", config);

    let devices = open_keyboard_devices(
        &["AT Translated Set 2 keyboard".to_string()],
        config.no_emit,
    )?;
    let virt_keyboard = create_virtual_keyboard()?;

    process(devices, virt_keyboard, config)?;

    Ok(())
}

fn process(mut device: EvDevDevice, mut virt_keyboard: UInputDevice, config: Config) -> Result<()> {
    let mut pending: HashMap<KeyCode, PendingKey> = HashMap::new();

    loop {
        let events = device.fetch_events()?;
        for ev in events {
            if ev.event_type() != EventType::KEY {
                continue;
            }
            let state = ev.value();
            let code = ev.code();
            let key = KeyCode(code);

            if state == PRESS {
                handle_press(
                    &mut virt_keyboard,
                    &config,
                    &mut pending,
                    key,
                )?;
            } else if state == RELEASE {
                handle_release(
                    &mut virt_keyboard,
                    &config,
                    &mut pending,
                    key,
                )?;
            }
        }
    }
}

fn handle_press(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(&remap) = config.remaps.get(&key) {
        if remap.hold.is_some() {
            pending.insert(
                key,
                PendingKey {
                    remap,
                    hold_sent: false,
                },
            );
        } else {
            press(virt_keyboard, remap.tap, config.no_emit)?;
        }
    } else {
        send_holds_for_pending_keys(virt_keyboard, config, pending)?;
        press(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}

fn send_holds_for_pending_keys(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
) -> Result<()> {
    for (_pending_keycode, pending_key) in pending.iter_mut() {
        let remap = pending_key.remap;
        if remap.hold.is_some()
            && !pending_key.hold_sent
            && remap.hold.is_some()
        {
            if let Some(hold_code) = remap.hold {
                press(virt_keyboard, hold_code, config.no_emit)?;
                pending_key.hold_sent = true;
            }
        }
    }
    Ok(())
}

fn handle_release(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(pending_key) = pending.remove(&key) {
        if pending_key.remap.hold.is_some() {
            if !pending_key.hold_sent {
                press(virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                release(virt_keyboard, pending_key.remap.tap, config.no_emit)?;
            } else if let Some(hold_code) = pending_key.remap.hold {
                release(virt_keyboard, hold_code, config.no_emit)?;
            }
        } else if pending_key.hold_sent {
            if let Some(hold_code) = pending_key.remap.hold {
                release(virt_keyboard, hold_code, config.no_emit)?;
            }
        } else if let Some(hold_code) = pending_key.remap.hold {
            press(virt_keyboard, hold_code, config.no_emit)?;
            release(virt_keyboard, hold_code, config.no_emit)?;
        }
    } else if let Some(&remap) = config.remaps.get(&key) {
        release(virt_keyboard, remap.tap, config.no_emit)?;
    } else {
        release(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}
