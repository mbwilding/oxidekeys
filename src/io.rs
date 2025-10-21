use crate::consts::*;
use crate::features::{Context, OutputEvent};
use anyhow::{Result, anyhow};
use colored::Colorize;
use evdev::KeyCode;
use log::debug;
use uinput::device::Device as UInputDevice;

pub fn create_virtual_keyboard(name: &str) -> Result<UInputDevice> {
    let device = uinput::default()
        .map_err(|e| anyhow!("Failed to open /dev/uinput (sudo modprobe uinput): {e}"))?
        .name(format!("{} OxideKeys", name))?
        .event(uinput::event::Keyboard::All)?
        .create()?;
    Ok(device)
}

pub fn emit(
    ctx: Context,
    device: &mut UInputDevice,
    events: Vec<OutputEvent>,
    feature_name: &'static str,
) -> Result<()> {
    for event in &events {
        match event {
            OutputEvent::Press(key) => {
                let key_reversed = ctx.device_config.layout.resolve_reverse(key);
                device.write(EV_KEY, key_reversed.0 as i32, PRESS)?;
                debug!(
                    "{}[{}] {:?} [{}]",
                    if is_modifier(key) { "    " } else { "" },
                    "↓".green().bold(),
                    key,
                    feature_name
                );
            }
            OutputEvent::Release(key) => {
                let key_reversed = ctx.device_config.layout.resolve_reverse(key);
                device.write(EV_KEY, key_reversed.0 as i32, RELEASE)?;
                debug!(
                    "{}[{}] {:?} [{}]",
                    if is_modifier(key) { "    " } else { "" },
                    "↑".red().bold(),
                    key,
                    feature_name.purple(),
                );
            }
            OutputEvent::PressMany(keys) => {
                for key in keys {
                    let key_reversed = ctx.device_config.layout.resolve_reverse(key);
                    device.write(EV_KEY, key_reversed.0 as i32, PRESS)?;
                    debug!(
                        "{}[{}] {:?} [{}]",
                        if is_modifier(key) { "    " } else { "" },
                        "↓".green().bold(),
                        key,
                        feature_name.purple(),
                    );
                }
            }
            OutputEvent::ReleaseMany(keys) => {
                for key in keys {
                    let key_reversed = ctx.device_config.layout.resolve_reverse(key);
                    device.write(EV_KEY, key_reversed.0 as i32, RELEASE)?;
                    debug!(
                        "{}[{}] {:?} [{}]",
                        if is_modifier(key) { "    " } else { "" },
                        "↑".red().bold(),
                        key,
                        feature_name.purple(),
                    );
                }
            }
        }
    }

    device.synchronize()?;

    Ok(())
}

pub fn emit_passthrough(
    ctx: Context,
    device: &mut UInputDevice,
    key: KeyCode,
    state: i32,
) -> Result<()> {
    let key_reversed = ctx.device_config.layout.resolve_reverse(&key);

    device.write(EV_KEY, key_reversed.0 as i32, state)?;
    device.synchronize()?;

    debug!(
        "{}[{}] {:?} [{}]",
        if is_modifier(&key) { "    " } else { "" },
        if state == PRESS {
            "↓".green().bold()
        } else {
            "↑".red().bold()
        },
        key,
        "raw".purple(),
    );

    Ok(())
}

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
