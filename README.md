# OxideKeys

## Overview

A utility to give you agency over your keyboards.
Event based, without polling.
Utilizes `uinput` for virtualizing the keyboard.

- **Remapping**: Remap keys
- **Dual function keys**: tap, hold remapping on a single key, overlap causes hold without delay times
- **Layers**: Hold a key and remap to anything
- **Home-Row Mods**: Allows setting home-row keys to be modifier keys on hold

## Install

```bash
cargo install --locked oxidekeys
```

## Config

Default config location: `~/.config/oxidekeys/config.yml`

You can also change the `hrm_term` per key, if not specified, it uses the global `hrm_term`.
