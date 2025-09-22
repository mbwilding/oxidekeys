use crate::consts::*;
use crate::features::OutputEvent;
use anyhow::{Result, anyhow};
use evdev::KeyCode;
use uinput::device::Device as UInputDevice;

pub fn create_virtual_keyboard(name: &str) -> Result<UInputDevice> {
    let device = uinput::default()
        .map_err(|e| anyhow!("Failed to open /dev/uinput (sudo modprobe uinput): {e}"))?
        .name(format!("{} OxideKeys", name))?
        .event(uinput::event::Keyboard::All)?
        .create()?;
    Ok(device)
}

pub fn emit(device: &mut UInputDevice, events: Vec<OutputEvent>, no_emit: bool) -> Result<()> {
    if no_emit {
        return Ok(());
    }
    for e in events {
        match e {
            OutputEvent::Press(k) => {
                device.write(EV_KEY, k.0 as i32, PRESS)?;
            }
            OutputEvent::Release(k) => {
                device.write(EV_KEY, k.0 as i32, RELEASE)?;
            }
            OutputEvent::PressMany(keys) => {
                for k in keys {
                    device.write(EV_KEY, k.0 as i32, PRESS)?;
                }
            }
            OutputEvent::ReleaseMany(keys) => {
                for k in keys {
                    device.write(EV_KEY, k.0 as i32, RELEASE)?;
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
    Ok(())
}
