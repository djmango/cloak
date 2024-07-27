CREATE TYPE keyboard_action_key_enum AS ENUM ('f1', 'f2', 'f3', 'f4', 'f5', 'f6', 'f7', 'f8', 'f9', 'f10', 'f11', 'f12', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'arrow_up', 'arrow_down', 'arrow_left', 'arrow_right', 'home', 'end', 'page_up', 'page_down', 'enter', 'escape', 'tab', 'space', 'backspace', 'insert', 'delete', 'num_lock', 'scroll_lock', 'pause', 'print_screen', 'grave', 'minus', 'equals', 'bracket_left', 'bracket_right', 'backslash', 'semicolon', 'quote', 'comma', 'period', 'slash');
CREATE TYPE modifier_key_enum AS ENUM ('caps_lock', 'shift', 'command', 'option', 'control', 'fn', 'alt', 'meta');
CREATE TYPE keyboard_action AS (
    key keyboard_action_key_enum,
    modifiers modifier_key_enum[]
);

ALTER TABLE devents DROP COLUMN keyboard_action;
ALTER TABLE devents ADD COLUMN keyboard_action keyboard_action;

DROP TYPE keyboard_action_enum;