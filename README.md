# OxideKeys

## Overview

A utility to give you agency over your keyboards.
Utilizes `uinput` for virtualizing the keyboards.

- **Remapping**: Remap keys
- **Dual function keys**: tap, hold remapping on a single key, overlap causes hold without delay times
- **Layers**: Hold a key and remap to anything
- **Home-Row Mods**: Allows setting home-row keys to be modifier keys on hold

## Install

```bash
cargo install --locked oxidekeys
```

## Setup

```bash
sudo usermod -aG input $USER
echo 'KERNEL=="uinput", MODE="0660", GROUP="input"' | sudo tee /etc/udev/rules.d/99-uinput.rules > /dev/null
echo uinput | sudo tee /etc/modules-load.d/uinput.conf > /dev/null
```

## Config

Default config location: `~/.config/oxidekeys/config.yml`

You can also change the `hrm_term` per key, if not specified, it uses the global `hrm_term`.
