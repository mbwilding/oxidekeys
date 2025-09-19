mod consts;
mod structs;

use crate::consts::*;
use crate::structs::*;
use anyhow::{Result, anyhow, bail};
use evdev::Device as EvDevDevice;
use evdev::{EventType, KeyCode};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use std::collections::HashMap;
use std::os::fd::AsFd;
use std::time::{Duration, Instant};
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
        let now = Instant::now();
        let next_expiry = pending
            .values()
            .filter(|pending_key| !pending_key.hold_sent)
            .filter_map(|pending_key| {
                if let Some(tt) = pending_key.remap.tapping_term_duration() {
                    let expiry = pending_key.start + tt;
                    if expiry > now { Some(expiry) } else { None }
                } else {
                    None
                }
            })
            .min();

        let timeout = next_expiry
            .map(|expiry| expiry.saturating_duration_since(now))
            .unwrap_or(Duration::from_secs(60));
        let poll_timeout = PollTimeout::try_from(timeout.as_millis())?;
        let fd = device.as_fd();
        let mut poll_fds = [PollFd::new(fd, PollFlags::POLLIN)];
        let poll_res = poll(&mut poll_fds, poll_timeout)?;

        // Handle timer holds
        for pending_key in pending.values_mut() {
            if !pending_key.hold_sent
                && let Some(tapping_term) = pending_key.remap.tapping_term
                && now.duration_since(pending_key.start)
                    > Duration::from_millis(tapping_term as u64)
                && let Some(hold_code) = pending_key.remap.hold
            {
                press(&mut virt_keyboard, hold_code, config.no_emit)?;
                pending_key.hold_sent = true;
            }
        }

        if poll_res > 0 {
            let events = device.fetch_events()?;
            for ev in events {
                if ev.event_type() != EventType::KEY {
                    continue;
                }
                let state = ev.value();
                let code = ev.code();
                let key = KeyCode(code);

                if state == PRESS {
                    if let Some(&remap) = config.remaps.get(&key) {
                        // Tap/hold/overlap logic
                        if remap.hold.is_some() && (remap.tapping_term.is_some() || remap.overlap) {
                            // Insert pending key
                            pending.insert(
                                key,
                                PendingKey {
                                    start: Instant::now(),
                                    remap,
                                    hold_sent: false,
                                },
                            );
                        } else {
                            press(&mut virt_keyboard, remap.tap, config.no_emit)?;
                        }
                    } else {
                        // Check for overlap triggers
                        for (_pending_keycode, pending_key) in pending.iter_mut() {
                            let remap = pending_key.remap;
                            if remap.overlap
                                && !pending_key.hold_sent
                                && let Some(hold_code) = remap.hold
                            {
                                press(&mut virt_keyboard, hold_code, config.no_emit)?;
                                pending_key.hold_sent = true;
                            }
                        }
                        press(&mut virt_keyboard, key, config.no_emit)?;
                    }
                } else if state == RELEASE {
                    if let Some(pending_key) = pending.remove(&key) {
                        if pending_key.remap.overlap {
                            // Overlap mode
                            if !pending_key.hold_sent {
                                // No overlap happened: tap
                                press(&mut virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                                release(&mut virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                            } else {
                                // If hold was sent, release hold
                                if let Some(hold_code) = pending_key.remap.hold {
                                    release(&mut virt_keyboard, hold_code, config.no_emit)?;
                                }
                            }
                        } else if let Some(tt) = pending_key.remap.tapping_term_duration() {
                            let elapsed = pending_key.start.elapsed();
                            if elapsed <= tt {
                                press(&mut virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                                release(&mut virt_keyboard, pending_key.remap.tap, config.no_emit)?;
                            } else if pending_key.hold_sent {
                                if let Some(hold_code) = pending_key.remap.hold {
                                    release(&mut virt_keyboard, hold_code, config.no_emit)?;
                                }
                            } else if let Some(hold_code) = pending_key.remap.hold {
                                press(&mut virt_keyboard, hold_code, config.no_emit)?;
                                release(&mut virt_keyboard, hold_code, config.no_emit)?;
                            }
                        }
                    } else if let Some(&remap) = config.remaps.get(&key) {
                        release(&mut virt_keyboard, remap.tap, config.no_emit)?;
                    } else {
                        release(&mut virt_keyboard, key, config.no_emit)?;
                    }
                }
            }
        }
    }
}
