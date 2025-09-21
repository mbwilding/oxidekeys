mod config;
mod consts;
mod keyboard;
mod structs;

use crate::{config::config, keyboard::*};
use anyhow::Result;
use std::thread;

fn main() -> Result<()> {
    env_logger::init();
    let config = config()?;
    let keyboards = open_keyboard_devices(&config)?;

    let mut handles = Vec::new();
    for keyboard in keyboards {
        let config = config.clone();

        let handle = thread::spawn(move || {
            process(keyboard, &config).unwrap();
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    Ok(())
}
