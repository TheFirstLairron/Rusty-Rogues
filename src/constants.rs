extern crate tcod;

pub const GAME_TITLE: &str = "TOMBS OF THE ANCIENT KINGS";
pub const FONT_PATH: &str = "arial10x10.png";
pub const SAVE_FILE_NAME: &str = "savegame";

pub mod gui {
    pub const SCREEN_WIDTH: i32 = 80;
    pub const SCREEN_HEIGHT: i32 = 50;

    pub const CHARACTER_SCREEN_WIDTH: i32 = 30;

    pub const INVENTORY_WIDTH: i32 = 50;

    pub const MAP_WIDTH: i32 = 80;
    pub const MAP_HEIGHT: i32 = 43;

    // sizes and coordinates relevant for the GUI
    pub const BAR_WIDTH: i32 = 20;
    pub const PANEL_HEIGHT: i32 = 7;
    pub const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

    // Message Log Constants
    pub const MSG_X: i32 = BAR_WIDTH + 2;
    pub const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
    pub const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

    pub const WELCOME_MESSAGE: &str =
        "Welcome stranger! Prepare to perish in the Tombs of the Ancient Kings.";

    pub mod menus {
        pub mod main {
            pub const MENU_NO_HEADER: &str = "";
            pub const GAME_CONSOLE_HEADER: &str = "Rusty Rogues";
            pub const AUTHOR_LINE: &str = "By Zach";
            pub const NEW_GAME: &str = "Play a new game";
            pub const CONTINUE: &str = "Continue last game";
            pub const QUIT: &str = "Quit";
            pub const IMAGE_PATH: &str = "menu_background.png";
            pub const START_MENU_WIDTH: i32 = 24;
        }

        pub mod level_up {
            pub const WIDTH: i32 = 40;
            pub const TITLE: &str = "Level up! Choose a stat to raise:\n";

            pub fn create_log_message(level: i32) -> String {
                format!(
                    "Your battle skills grow stronger! You reached level {}",
                    level
                )
            }

            pub fn create_constitution_option(base: i32) -> String {
                format!("Constitution (+20 HP, from {})", base)
            }

            pub fn create_stength_option(base: i32) -> String {
                format!("Strength (+1 attack, from {})", base)
            }

            pub fn create_agility_option(base: i32) -> String {
                format!("Agility (+1 defense, from {})", base)
            }
        }

        pub mod next_level {
            use tcod::colors::{self, Color};
            pub const REST_LOG_MESSAGE: &str =
                "You take a moment to rest and recover your strength.";
            pub const NEXT_LEVEL_LOG_MESSAGE: &str =
                "After a rare moment of peace, you descend deeper into the heart of the dungeon.";
            pub const REST_COLOR: Color = colors::VIOLET;
            pub const NEXT_LEVEL_COLOR: Color = colors::RED;

        }

        pub mod character_sheet {}

        pub mod inventory {}

        pub mod drop {}
    }
}

pub mod player_base {
    use crate::colors::{self, Color};

    pub const NAME: &str = "Player";
    pub const SYMBOL: char = '@';
    pub const COLOR: Color = colors::WHITE;
}

pub mod gear {
    pub mod dagger {
        use crate::colors::{self, Color};

        pub const NAME: &str = "Dagger";
        pub const SYMBOL: char = '-';
        pub const COLOR: Color = colors::SKY;
        pub const HP_BONUS: i32 = 0;
        pub const DEFENSE_BONUS: i32 = 0;
        pub const POWER_BONUS: i32 = 2;
    }

    pub mod iron_sword {}

    pub mod shield {}
}

pub mod consumables {
    pub mod potions {
        pub mod healing {}
    }

    pub mod scrolls {
        pub mod lightning {}

        pub mod confusion {
        }

        pub mod fireball {
            use tcod::colors::{self, Color};

            pub const RADIUS: i32 = 3;
            pub const RADIUS_COLOR: Color = colors::ORANGE;

            pub const DAMAGE: i32 = 25;
            pub const DAMAGE_COLOR: Color = colors::ORANGE;

            pub const INSTRUCTIONS: &str = "Left-click a target tile for the fireball, or right-click to cancel.";
            pub const INSTRUCTION_COLOR: Color = colors::LIGHT_CYAN;

            pub fn create_radius_message() -> String {
                format!(
                    "The fireball explodes, burning everything within {} tiles!",
                    RADIUS
                )
            }

            pub fn create_damage_message(name: &String) -> String {
                format!("The {} gets burned for {} hit points.", name, DAMAGE)
            }
        }
    }
}
