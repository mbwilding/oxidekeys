# Keyflect

Default config location: `~/.config/keyflect/config.yml`

```yml
no_emit: false
hrm_term: 130
keyboards:
  AT Translated Set 2 keyboard:
    KEY_CAPSLOCK:
      tap: KEY_BACKSPACE
    KEY_S:
      tap: KEY_S
      hold: KEY_LEFTMETA
      hrm: true
    KEY_K:
      tap: KEY_K
      hold: KEY_RIGHTALT
      hrm: true
    KEY_SPACE:
      tap: KEY_SPACE
      hold: KEY_LEFTSHIFT
    KEY_LEFTSHIFT:
      tap: KEY_ESC
    KEY_SEMICOLON:
      tap: KEY_SEMICOLON
      hold: KEY_RIGHTCTRL
      hrm: true
    KEY_L:
      tap: KEY_L
      hold: KEY_RIGHTMETA
      hrm: true
    KEY_D:
      tap: KEY_D
      hold: KEY_LEFTALT
      hrm: true
    KEY_A:
      tap: KEY_A
      hold: KEY_LEFTCTRL
      hrm: true
layers:
  Navigation:
    KEY_RIGHTALT:
      KEY_V: KEY_UP
      KEY_C: KEY_DOWN
      KEY_P: KEY_RIGHT
      KEY_J: KEY_LEFT
```

You can also change the `hrm_term` per key, if not specified, it uses the global `hrm_term`.
