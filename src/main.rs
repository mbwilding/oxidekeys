mod config;
mod consts;
mod features;
mod io;
mod keyboard;
mod pipeline;

use crate::{
    config::config,
    keyboard::{keyboard_processor, open_keyboard_devices},
};
use anyhow::Result;
use std::thread;

fn main() -> Result<()> {
    env_logger::init();
    let config = config()?;
    let keyboards = open_keyboard_devices(&config)?;

    if keyboards.len() > 1 {
        if let Some(keyboard) = keyboards.into_iter().nth(0) {
            if let Err(e) = keyboard_processor(keyboard, &config) {
                eprintln!("Error processing keyboard: {}", e);
                return Err(e);
            }
        } else {
            eprintln!("No keyboard found in the list.");
            return Err(anyhow::anyhow!("No keyboard found"));
        }
    } else {
        let mut handles = Vec::new();

        for keyboard in keyboards {
            let config = config.clone();
            let handle = thread::spawn(move || {
                if let Err(e) = keyboard_processor(keyboard, &config) {
                    eprintln!("Thread error processing keyboard: {}", e);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            if let Err(e) = handle.join() {
                eprintln!("Thread join error: {:?}", e);
                return Err(anyhow::anyhow!("Thread join error: {:?}", e));
            }
        }
    }

    Ok(())
}
