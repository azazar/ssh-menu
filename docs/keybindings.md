# Keybindings

All app shortcuts can be changed under `keybindings` in `~/.ssh/ssh-menu-config.yml`. Each screen has its own action map: `menu`, `dialog`, `help`, and `editor`. Omitted actions retain their defaults. A supplied list replaces the defaults for that action; `[]` disables it. Key names are case-insensitive.

```yaml
keybindings:
  menu:
    reveal_alt: [Alt+?]
    reveal_ctrl_alt: [Ctrl+Alt+?]
    reveal_ctrl: [Ctrl+?]
    connect: [Enter]
    connect_and_return: [Alt+Enter]
    connection_settings: [Ctrl+Enter]
    help: [F1]
    new_remote: [F2]
    edit_config: [F4]
    toggle_transport: [F8]
    quit: [Esc, Ctrl+C]
    up: [Up]
    down: [Down]
    page_up: [PgUp]
    page_down: [PgDn]
    left: [Left]
    right: [Right]
    erase_character: [Backspace, Delete]
    erase_word: [Ctrl+Backspace, Ctrl+Delete]
  dialog:
    connect: [Enter]
    connect_and_return: [Alt+Enter]
    help: [F1]
    cancel: [Esc, Ctrl+C]
    next_field: [Tab, Down]
    previous_field: [Shift+Tab, Up]
    left: [Left]
    right: [Right]
    erase_character: [Backspace, Delete]
    erase_word: [Ctrl+Backspace, Ctrl+Delete]
  help:
    help: [F1] # Toggle help closed
    close: [Esc, Ctrl+C, Q]
    next_topic: [Right, Tab]
    previous_topic: [Left, Shift+Tab]
    up: [Up]
    down: [Down]
    page_up: [PgUp]
    page_down: [PgDn, Space]
    home: [Home]
    end: [End]
  editor:
    help: [F1]
    edit: [Enter, F4]
    cancel: [Esc, Ctrl+C]
```

`connect` hands the terminal to the selected connection. `connect_and_return` waits for the command to exit and reopens the same menu, preserving the filter and profile choices. It works both from the host list and when confirming a connection or new-remote dialog. If invoked from the host list on a profile requiring inputs, returning remains enabled when you confirm that dialog. Profile shortcuts still perform ordinary connections; use the return action with the selected profile when desired.

The old top-level `return_to_menu` option has been removed. Delete that line from existing configurations and bind `connect_and_return` instead. For example, to make Enter return and Alt+Enter hand off, swap their lists under `menu` and `dialog`.

Supported keys: letters with `Ctrl+`, `Alt+`, or `Ctrl+Alt+`; `F1`–`F24`; `Enter`, `Esc`, `Tab`, `Shift+Tab`, `Backspace`, `Delete`, arrows, `PgUp`, `PgDn`, `Home`, `End`, and `Space`. Named keys also accept Ctrl/Alt/Shift combinations. Shift-modified text characters are not supported. Unmodified text keys are allowed only in the help viewer so typing continues to filter hosts or edit fields.

Duplicate keys in a screen, unknown actions, and conflicts with profile shortcuts are rejected. Move or disable the existing action before reusing its key. Profile shortcuts remain modified letters in each profile's `key` field; Ctrl+C can be assigned there after removing it from `menu.quit`. Terminal aliases are checked too: Ctrl+I is Tab, Ctrl+J/Ctrl+M is Enter, and Ctrl+H is Ctrl+Backspace. Some terminals cannot distinguish modified Enter or other combinations; bind a function key or modified letter instead.

`menu.edit_config` launches the external editor immediately. The `editor` bindings apply to the recovery screen shown only when editing or validation fails.

Footer hints show the current bindings. The **Keys** topic in built-in help lists every active binding for the current screen and help viewer, including disabled actions. Changes saved through the configuration editor take effect immediately after validation.

## Modifier hints

Every screen also supports `reveal_alt: [Alt+?]`, `reveal_ctrl_alt: [Ctrl+Alt+?]` and `reveal_ctrl: [Ctrl+?]` by default. These actions toggle modifier-specific hints for terminals without modifier-only events; put overrides in `menu`, `dialog`, `help` or `editor` as appropriate. In terminals with Kitty keyboard support, simply hold the modifiers to reveal their exact group and release them to restore the normal bar. Idle prompts occupy the same single row as other controls. The bar uses compact labels (`C-`, `A-`, `S-`, `↵`); the Keys topic lists full bindings and all available profile shortcuts.
