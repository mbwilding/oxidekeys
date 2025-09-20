mod consts;
mod structs;

use crate::consts::*;
use crate::structs::*;
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use std::time::Instant;
use udev::Enumerator;
use uinput::device::Device as UInputDevice;

fn open_keyboard_devices(
    config: &Config,
) -> Result<Vec<(EvDevDevice, HashMap<KeyCode, RemapAction>)>> {
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
                Some(name_value) => config
                    .keyboards
                    .iter()
                    .any(|keyboard| name_value == keyboard.0),
                None => false,
            };

            if name_matches {
                println!("Keyboard Monitored: {:?}", dev.name());

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
        .map_err(|e| anyhow!("Failed to open /dev/uinput (sudo modprobe uinput): {e}"))?
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
    let config_path = dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("interception-rust")
        .join("config.yml");

    let config: Config;
    if !config_path.exists() {
        config = Config::default();
        fs::create_dir_all(config_path.parent().unwrap())?;
        let config_yaml = serde_yaml::to_string(&config)?;
        fs::write(&config_path, config_yaml)?;
        println!("Default config written to {}", config_path.display());
    } else {
        let config_content = fs::read_to_string(&config_path)?;
        config = serde_yaml::from_str(&config_content)?;
    }
    println!("Config: {:#?}", config);

    let devices = open_keyboard_devices(&config)?;
    let virt_keyboard = Arc::new(Mutex::new(create_virtual_keyboard()?));

    let mut handles = Vec::new();
    for device in devices {
        let virt_keyboard = virt_keyboard.clone();
        let ev_dev_device = device.0;
        let remaps = device.1.clone();
        let config = config.clone();
        let handle = thread::spawn(move || {
            process(ev_dev_device, virt_keyboard, &remaps, &config).unwrap();
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}

fn process(
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

            if state == PRESS {
                handle_press(&mut virt_keyboard, config, remaps, &mut pending, key)?;
            } else if state == RELEASE {
                handle_release(&mut virt_keyboard, config, remaps, &mut pending, key)?;
            }
        }
    }
}

fn handle_press(
    virt_keyboard: &mut UInputDevice,
    config: &Config,
    remaps: &HashMap<KeyCode, RemapAction>,
    pending: &mut HashMap<KeyCode, PendingKey>,
    key: KeyCode,
) -> Result<()> {
    if let Some(&remap) = remaps.get(&key) {
        if remap.hold.is_some() {
            pending.insert(
                key,
                PendingKey {
                    remap,
                    hold_sent: false,
                    time_pressed: Instant::now(),
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
    remaps: &HashMap<KeyCode, RemapAction>,
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
    } else if let Some(&remap) = remaps.get(&key) {
        release(virt_keyboard, remap.tap, config.no_emit)?;
    } else {
        release(virt_keyboard, key, config.no_emit)?;
    }
    Ok(())
}
