extern crate tcod;

use tcod::colors::{self, Color};
use tcod::console::*;
use tcod::input::Mouse;
use tcod::map::Map as FovMap;

use crate::constants::game as GameConstants;
use crate::constants::gui as GuiConstants;
use crate::game_objects::{Game, GameObject};
use crate::tcod_container::Tcod;

fn get_names_under_mouse(mouse: Mouse, objects: &[GameObject], fov_map: &FovMap) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);

    let names = objects
        .iter()
        .filter(|obj| obj.pos() == (x, y) && fov_map.is_in_fov(obj.x, obj.y))
        .map(|obj| obj.name.clone())
        .collect::<Vec<_>>();

    names.join(", ")
}

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
) {
    // Calculate the width of the bar
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // Render the background
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // Render the Bar
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    panel.set_default_foreground(colors::WHITE);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {}/{}", name, value, maximum),
    );
}

pub fn msgbox(text: &str, width: i32, mut tcod: &mut Tcod) {
    let options: &[&str] = &[];
    menu(text, options, width, &mut tcod);
}

pub fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32, tcod: &mut Tcod) -> Option<usize> {
    assert!(
        options.len() <= 26,
        "Cannot have a menu with more than 26 options"
    );

    // calculate total height for the header (after auto-wrap) and one line per option
    let header_height = if header.is_empty() {
        0
    } else {
        tcod.root
            .get_height_rect(0, 0, width, GuiConstants::SCREEN_HEIGHT, header)
    };

    let height = options.len() as i32 + header_height;

    let mut window = Offscreen::new(width, height);

    // print the header, with auto-wrap;
    window.set_default_foreground(colors::WHITE);
    window.print_rect_ex(
        0,
        0,
        width,
        height,
        BackgroundFlag::None,
        TextAlignment::Left,
        header,
    );

    // print all the options
    for (index, option_text) in options.iter().enumerate() {
        // essentially ASCII math, probably a better way of approaching this entire menu
        let menu_letter = (b'a' + index as u8) as char;
        let text = format!("({}) {}", menu_letter, option_text.as_ref());
        window.print_ex(
            0,
            header_height + index as i32,
            BackgroundFlag::None,
            TextAlignment::Left,
            text,
        );
    }

    let x = GuiConstants::SCREEN_WIDTH / 2 - width / 2;
    let y = GuiConstants::SCREEN_HEIGHT / 2 - height / 2;
    tcod::console::blit(
        &window,
        (0, 0),
        (width, height),
        &mut tcod.root,
        (x, y),
        1.0,
        0.7,
    );

    // present the root console to the player and wait for a key press
    tcod.root.flush();
    let key = tcod.root.wait_for_keypress(true);

    // convert the ASCII code to an index; if it corresponds to an option, return it
    if key.printable.is_alphabetic() {
        let index = key.printable.to_ascii_lowercase() as usize - 'a' as usize;
        if index < options.len() {
            Some(index)
        } else {
            None
        }
    } else {
        None
    }
}

pub fn inventory_menu(game: &Game, header: &str, tcod: &mut Tcod) -> Option<usize> {
    let options = if game.inventory.is_empty() {
        vec!["Inventory is empty.".into()]
    } else {
        game.inventory
            .iter()
            .map(|item| match item.equipment {
                Some(equipment) if equipment.equipped => {
                    format!("{} (on {})", item.name, equipment.slot)
                }
                _ => item.name.clone(),
            })
            .collect()
    };

    let inventory_index = menu(header, &options, GuiConstants::INVENTORY_WIDTH, tcod);

    // if an item was chosen, return it
    if !game.inventory.is_empty() {
        inventory_index
    } else {
        None
    }
}

pub fn render_all(tcod: &mut Tcod, game_objects: &[GameObject], game: &mut Game) {
    // originally checked if user moved, but that caused a bug: every action was delayed by one turn. No observable adverse effects from removing the check.
    let player = &game_objects[GameConstants::PLAYER];
    tcod.fov.compute_fov(
        player.x,
        player.y,
        GameConstants::TORCH_RADIUS,
        GameConstants::FOV_LIGHT_WALLS,
        GameConstants::FOV_ALGO,
    );

    // Go through all tiles and set their background color
    for y in 0..GuiConstants::MAP_HEIGHT {
        for x in 0..GuiConstants::MAP_WIDTH {
            // check if it's a wall by checking if it blocks sight
            let visible = tcod.fov.is_in_fov(x, y);
            let is_wall = game.map[x as usize][y as usize].block_sight;
            let color = match (visible, is_wall) {
                // Outside FOV
                (false, true) => GameConstants::COLOR_DARK_WALL,
                (false, false) => GameConstants::COLOR_DARK_GROUND,
                // Inside FOV
                (true, true) => GameConstants::COLOR_LIGHT_WALL,
                (true, false) => GameConstants::COLOR_LIGHT_GROUND,
            };

            let explored = &mut game.map[x as usize][y as usize].explored;

            if visible {
                *explored = true;
            }

            if *explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // Draw the GameObjects
    let mut to_draw: Vec<_> = game_objects
        .iter()
        .filter(|item| {
            tcod.fov.is_in_fov(item.x, item.y)
                || (item.always_visible && game.map[item.x as usize][item.y as usize].explored)
        })
        .collect();
    // Sort so that non-blocking objets come first
    to_draw.sort_by(|item1, item2| item1.blocks.cmp(&item2.blocks));
    // Draw the items in the list
    for object in to_draw {
        // only render if in FOV
        object.draw(&mut tcod.con);
    }

    // Blit onto the actual screen
    blit(
        &tcod.con,
        (0, 0),
        (GuiConstants::SCREEN_WIDTH, GuiConstants::SCREEN_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    tcod.panel.set_default_background(colors::BLACK);
    tcod.panel.clear();

    // Print the game messages, one line at a time
    let mut y = GuiConstants::MSG_HEIGHT as i32;

    for &(ref msg, color) in game.log.iter().rev() {
        let msg_height =
            tcod.panel
                .get_height_rect(GuiConstants::MSG_X, y, GuiConstants::MSG_WIDTH, 0, msg);
        y -= msg_height;

        if y < 0 {
            break;
        }

        tcod.panel.set_default_foreground(color);
        tcod.panel
            .print_rect(GuiConstants::MSG_X, y, GuiConstants::MSG_WIDTH, 0, msg);
    }

    // Show the players stats
    let player = &game_objects[GameConstants::PLAYER];
    let hp = player.fighter.map_or(0, |f| f.hp);
    let max_hp = player.max_hp(game);

    render_bar(
        &mut tcod.panel,
        1,
        1,
        GuiConstants::BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        colors::LIGHT_RED,
        colors::DARKER_RED,
    );

    tcod.panel.print_ex(
        1,
        3,
        BackgroundFlag::None,
        TextAlignment::Left,
        format!("Dungeon Level: {}", game.dungeon_level),
    );

    // Display the names of the objects under th mouse
    tcod.panel.set_default_foreground(colors::LIGHT_GREY);
    tcod.panel.print_ex(
        1,
        0,
        BackgroundFlag::None,
        TextAlignment::Left,
        get_names_under_mouse(tcod.mouse, game_objects, &tcod.fov),
    );

    blit(
        &tcod.panel,
        (0, 0),
        (GuiConstants::SCREEN_WIDTH, GuiConstants::PANEL_HEIGHT),
        &mut tcod.root,
        (0, GuiConstants::PANEL_Y),
        1.0,
        1.0,
    );

    // Make the console actually visible
    tcod.root.flush();
}
