mod consts;
mod keyboard;
mod structs;

use crate::keyboard::*;
use crate::structs::*;
use anyhow::Result;
use std::fs;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

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
