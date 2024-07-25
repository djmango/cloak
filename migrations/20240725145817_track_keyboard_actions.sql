CREATE TYPE keyboard_action_key_enum AS ENUM ('shift', 'ctrl', 'alt', 'meta', 'f1', 'f2', 'f3', 'f4', 'f5', 'f6', 'f7', 'f8', 'f9', 'f10', 'f11', 'f12', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'num_0', 'num_1', 'num_2', 'num_3', 'num_4', 'num_5', 'num_6', 'num_7', 'num_8', 'num_9', 'arrow_up', 'arrow_down', 'arrow_left', 'arrow_right', 'home', 'end', 'page_up', 'page_down', 'enter', 'escape', 'tab', 'space', 'backspace', 'insert', 'delete', 'caps_lock', 'num_lock', 'scroll_lock', 'pause', 'print_screen');
CREATE TYPE keyboard_action AS (
    action keyboard_action_key_enum,
    duration INTEGER
);

ALTER TABLE devents DROP COLUMN keyboard_action;
ALTER TABLE devents ADD COLUMN keyboard_action keyboard_action;

DROP TYPE keyboard_action_enum;