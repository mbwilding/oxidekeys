use crate::config::{Config, KeyboardConfig};
use crate::features::{
    layers::LayersFeature,
    overlaps::OverlapsFeature,
};
use crate::io::create_virtual_keyboard;
use crate::pipeline::Pipeline;
use crate::state::Pending;
use anyhow::{Result, bail};
use crossbeam_channel::{select, unbounded};
use evdev::Device as EvDevDevice;
use evdev::{EventType, InputEvent, KeyCode};
use log::{debug, info};
use std::collections::HashSet;
use udev::Enumerator;

pub(crate) struct Keyboard {
    pub device: EvDevDevice,
    pub config: KeyboardConfig,
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

pub(crate) fn keyboard_processor(keyboard: Keyboard, config: &Config) -> Result<()> {
    let mut virt_keyboard = create_virtual_keyboard(keyboard.device.name().unwrap())?;
    let mut device = keyboard.device;
    let kcfg = keyboard.config;
    let mut pending: Pending = Default::default();
    let mut keys_down: HashSet<KeyCode> = HashSet::new();
    let mut active_layers: HashSet<String> = HashSet::new();

    // let mut hrm = HrmFeature::new();
    // let timer_rx: Receiver<TimerMsg> = hrm.receiver();
    let mut pipeline = Pipeline::new(vec![
        Box::new(LayersFeature::new()),
        Box::new(OverlapsFeature::new()),
        // Box::new(hrm),
    ]);

    let (tx, rx) = unbounded::<InputEvent>();

    // Producer thread: blocks on fetch_events and sends to channel
    std::thread::spawn(move || {
        loop {
            match device.fetch_events() {
                Err(_) => {
                    // Producer ends on error; consumer will observe disconnect
                    break;
                }
                Ok(events) => {
                    for event in events {
                        // Best-effort send; if consumer dropped, exit
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    // Consumer: multiplex input events and HRM timer
    loop {
        select! {
            recv(rx) -> ev => {
                let event = match ev { Ok(e) => e, Err(_) => break };
                if event.event_type() != EventType::KEY { continue; }
                let state = event.value();
                let key = KeyCode(event.code());
                pipeline.process_event(
                    &mut virt_keyboard,
                    config,
                    &kcfg,
                    &mut pending,
                    &mut keys_down,
                    &mut active_layers,
                    key,
                    state,
                )?;
            }
            // recv(timer_rx) -> msg => {
            //     if let Ok(TimerMsg::HoldTimeout(key)) = msg {
            //         pipeline.process_timer(
            //             &mut virt_keyboard,
            //             config,
            //             &kcfg,
            //             &mut pending,
            //             &mut keys_down,
            //             &mut active_layers,
            //             key,
            //         )?;
            //     }
            // }
        }
    }

    Ok(())
}
