use crate::{
    config::{Config, KeyboardConfig},
    consts::*,
    features::{layers::LayersFeature, overlaps::OverlapsFeature},
    io::create_virtual_keyboard,
    pipeline::Pipeline,
};
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
    let kb_config = keyboard.config;
    let mut keys_down: HashSet<KeyCode> = HashSet::new();
    let mut active_layers: HashSet<String> = HashSet::new();

    let mut features: Vec<Box<dyn crate::features::Feature + Send>> = Vec::new();

    if *config.features.get("overlaps").unwrap_or(&true) {
        features.push(Box::new(OverlapsFeature::new()));
    }

    if *config.features.get("layers").unwrap_or(&true) {
        features.push(Box::new(LayersFeature::new()));
    }

    let mut pipeline = Pipeline::new(features);

    let (tx, rx) = unbounded::<InputEvent>();

    std::thread::spawn(move || {
        loop {
            match device.fetch_events() {
                Err(_) => {
                    break;
                }
                Ok(events) => {
                    for event in events {
                        if tx.send(event).is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });

    loop {
        select! {
            recv(rx) -> ev => {
                let event = match ev { Ok(e) => e, Err(_) => break };
                if event.event_type() != EventType::KEY { continue; }
                let state = event.value();
                if state > PRESS { continue; }
                let key = KeyCode(event.code());
                pipeline.process_event(
                    &mut virt_keyboard,
                    config,
                    &kb_config,
                    &mut keys_down,
                    &mut active_layers,
                    key,
                    state,
                )?;
            }
        }
    }

    Ok(())
}
