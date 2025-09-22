use crate::consts::*;
use crate::features::OutputEvent;
use anyhow::{Result, anyhow};
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
    device: &mut UInputDevice,
    events: Vec<OutputEvent>,
    no_emit: bool,
    feature_name: &'static str,
) -> Result<()> {
    if no_emit {
        return Ok(());
    }

    for event in &events {
        match event {
            OutputEvent::Press(k) => {
                device.write(EV_KEY, k.0 as i32, PRESS)?;
                debug!("{:?} [PRESS] [{}]", k, feature_name);
            }
            OutputEvent::Release(k) => {
                device.write(EV_KEY, k.0 as i32, RELEASE)?;
                debug!("{:?} [RELEASE] [{}]", k, feature_name);
            }
            OutputEvent::PressMany(keys) => {
                for k in keys {
                    device.write(EV_KEY, k.0 as i32, PRESS)?;
                    debug!("{:?} [PRESS] [{}]", k, feature_name);
                }
            }
            OutputEvent::ReleaseMany(keys) => {
                for k in keys {
                    device.write(EV_KEY, k.0 as i32, RELEASE)?;
                    debug!("{:?} [RELEASE] [{}]", k, feature_name);
                }
            }
        }
    }
    device.synchronize()?;
    Ok(())
}

pub fn emit_passthrough(
    device: &mut UInputDevice,
    key: KeyCode,
    state: i32,
    no_emit: bool,
) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    device.write(EV_KEY, key.0 as i32, state)?;
    device.synchronize()?;
    debug!(
        "{:?} [{}] [raw]",
        key,
        if state == PRESS { "PRESS" } else { "RELEASE" }
    );
    Ok(())
}
