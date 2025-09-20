mod consts;
mod structs;

use crate::consts::*;
use crate::structs::*;
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use udev::Enumerator;
use uinput::device::Device as UInputDevice;

fn open_keyboard_devices(config: &Config) -> Result<Vec<(EvDevDevice, HashMap<KeyCode, RemapAction>)>> {
    println!("Detecting keyboards");

    let mut enumerator = Enumerator::new()?;
    enumerator.match_subsystem("input")?;
    enumerator.match_property("ID_INPUT_KEYBOARD", "1")?;

    let mut devices = Vec::new();

    for device in enumerator.scan_devices()? {
        if let Some(devnode) = device.devnode()
            && let Ok(mut dev) = EvDevDevice::open(devnode)
        {
            let name_matches = match dev.name() {
                Some(name_value) => config.keyboards.iter().any(|keyboard| name_value == keyboard.0),
                None => false,
            };

            if name_matches {
                println!("Keyboard Monitored: {:?}", dev.name());
                if !config.no_emit {
                    dev.grab()?;
                }
                // Find the associated keyboard value
                let keyboard_value = dev.name().and_then(|name_value| {
                    config.keyboards.iter()
                        .find_map(|(k, v)| if name_value == k { Some(v.clone()) } else { None })
                }).unwrap_or_default();
                devices.push((dev, keyboard_value));
            } else {
                println!("Keyboard Ignored: {:?}", dev.name());
            }
        }
    }

    if devices.is_empty() {
        bail!("No keyboards found");
    } else {
        Ok(devices)
    }
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

    let devices = open_keyboard_devices(&config)?;
    let virt_keyboard = Arc::new(Mutex::new(create_virtual_keyboard()?));

    for device in devices {
        process(device.0, virt_keyboard.clone(), &device.1, &config)?;
    }

    Ok(())
}

fn process(mut device: EvDevDevice, virt_keyboard: Arc<Mutex<UInputDevice>>, keyboard: &HashMap<KeyCode, RemapAction>, config: &Config) -> Result<()> {
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

            if state == PRESS {
                handle_press(&mut virt_keyboard, &config, keyboard, &mut pending, key)?;
            } else if state == RELEASE {
                handle_release(&mut virt_keyboard, &config, keyboard, &mut pending, key)?;
            }
        }
    }
}

fn handle_press(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    keyboard: &HashMap<KeyCode, RemapAction>,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(&remap) = keyboard.get(&key) {
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
            && let Some(hold_code) = remap.hold
        {
            press(virt_keyboard, hold_code, config.no_emit)?;
            pending_key.hold_sent = true;
        }
    }
    Ok(())
}

fn handle_release(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    keyboard: &HashMap<KeyCode, RemapAction>,
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
    } else if let Some(&remap) = keyboard.get(&key) {
        release(virt_keyboard, remap.tap, config.no_emit)?;
    } else {
        release(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}
