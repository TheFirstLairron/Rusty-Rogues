#[macro_use]
extern crate serde_derive;

extern crate rand;
extern crate tcod;

mod constants;
mod game_objects;
mod enemies;
mod map;
mod spell_effects;
mod tcod_container;
mod items;
mod render;

use tcod::colors;
use tcod::console::*;
use tcod::input::Key;
use tcod::input::KeyCode::*;
use tcod::input::{self, Event};
use tcod::map::Map as FovMap;

use std::cmp;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use rand::Rng;

use constants::game as GameConstants;
use constants::gui as GuiConstants;

use game_objects::Game as Game;
use game_objects::GameObject as GameObject;
use game_objects::Fighter as Fighter;
use game_objects::DeathCallback as DeathCallback;
use game_objects::Ai as Ai;
use game_objects::Item as Item;
use game_objects::Equipment as Equipment;
use game_objects::Slot as Slot;
use game_objects::MessageLog as MessageLog;

use map::create_map as create_map;

use tcod_container::Tcod as Tcod;

#[derive(Clone, Copy, Debug, PartialEq)]
enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

fn handle_keys(
    key: Key,
    mut tcod: &mut Tcod,
    mut game: &mut Game,
    objects: &mut Vec<GameObject>,
) -> PlayerAction {
    use PlayerAction::*;

    let player_alive = objects[GameConstants::PLAYER].alive;

    match (key, player_alive) {
        (Key { code: Up, .. }, true) | (Key { code: NumPad8, .. }, true) => {
            player_move_or_attack(0, -1, game, objects);
            TookTurn
        }
        (Key { code: Down, .. }, true) | (Key { code: NumPad2, .. }, true) => {
            player_move_or_attack(0, 1, game, objects);
            TookTurn
        }
        (Key { code: Left, .. }, true) | (Key { code: NumPad4, .. }, true) => {
            player_move_or_attack(-1, 0, game, objects);
            TookTurn
        }
        (Key { code: Right, .. }, true) | (Key { code: NumPad6, .. }, true) => {
            player_move_or_attack(1, 0, game, objects);
            TookTurn
        }
        (Key { code: Home, .. }, true) | (Key { code: NumPad7, .. }, true) => {
            player_move_or_attack(-1, -1, game, objects);
            TookTurn
        }
        (Key { code: PageUp, .. }, true) | (Key { code: NumPad9, .. }, true) => {
            player_move_or_attack(1, -1, game, objects);
            TookTurn
        }
        (Key { code: End, .. }, true) | (Key { code: NumPad1, .. }, true) => {
            player_move_or_attack(-1, 1, game, objects);
            TookTurn
        }
        (Key { code: PageDown, .. }, true) | (Key { code: NumPad3, .. }, true) => {
            player_move_or_attack(1, 1, game, objects);
            TookTurn
        }
        (Key { code: NumPad5, .. }, true) => {
            TookTurn // do nothing, i.e. wait for the monster to come to you
        }
        (Key { printable: 'g', .. }, true) => {
            // pick up an item
            let item_id = objects
                .iter()
                .position(|object| object.pos() == objects[GameConstants::PLAYER].pos() && object.item.is_some());

            if let Some(item_id) = item_id {
                pick_item_up(item_id, objects, game);
            }

            DidntTakeTurn
        }
        (Key { printable: 'i', .. }, true) => {
            // show the inventory: if an item is selected, use it
            let inventory_index = inventory_menu(
                game,
                "Press the key next to an item to use it, or any other to cancel. \n",
                &mut tcod,
            );

            if let Some(inventory_index) = inventory_index {
                use_item(inventory_index, objects, tcod, game)
            }

            DidntTakeTurn
        }
        (Key { printable: 'd', .. }, true) => {
            // show the inventory; if an item is selected, drop it
            let inventory_index = inventory_menu(
                game,
                "Press the key next to an item to drop it, or any other to cancel.\n",
                &mut tcod,
            );
            if let Some(inventory_index) = inventory_index {
                drop_item(inventory_index, &mut game, objects);
            }
            DidntTakeTurn
        }
        (Key { printable: 'c', .. }, true) => {
            // show character information
            let player = &objects[GameConstants::PLAYER];
            let level = player.level;
            let level_up_xp = GameConstants::LEVEL_UP_BASE + player.level * GameConstants::LEVEL_UP_FACTOR;
            if let Some(fighter) = player.fighter.as_ref() {
                let msg = format!(
                    "Character Information: \n* Level: {} \n* Experience: {} \n* Experience to level up: {} \n\n* Maximum HP: {} \n* Attack: {} \n* Defense: {} \n",
                    level, fighter.xp, level_up_xp, player.max_hp(game), player.power(game), player.defense(game)
                );
                msgbox(&msg, GuiConstants::CHARACTER_SCREEN_WIDTH, &mut tcod);
            }

            DidntTakeTurn
        }
        (Key { printable: '<', .. }, true) => {
            // go down the stairs if the player is on them
            let player_on_stairs = objects
                .iter()
                .any(|object| object.pos() == objects[GameConstants::PLAYER].pos() && object.name == "stairs");

            if player_on_stairs {
                next_level(tcod, objects, game);
            }
            DidntTakeTurn
        }
        (
            Key {
                code: Enter,
                alt: true,
                ..
            },
            _,
        ) => {
            let fullscreen = tcod.root.is_fullscreen();
            tcod.root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        }
        (Key { code: Escape, .. }, _) => Exit,
        _ => DidntTakeTurn,
    }
}

fn move_by(id: usize, dx: i32, dy: i32, game: &mut Game, objects: &mut [GameObject]) {
    let (x, y) = objects[id].pos();

    if !map::is_blocked(x + dx, y + dy, &game.map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn move_towards(
    id: usize,
    target_x: i32,
    target_y: i32,
    mut game: &mut Game,
    objects: &mut [GameObject],
) {
    // Vector from this object to the target and distance
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    // Normalize it to length 1 (preserving direction), then round it and convert to int so the movement is restricted to the grid
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, &mut game, objects);
}

fn player_move_or_attack(dx: i32, dy: i32, mut game: &mut Game, objects: &mut [GameObject]) {
    let x = objects[GameConstants::PLAYER].x + dx;
    let y = objects[GameConstants::PLAYER].y + dy;

    let target_id = objects
        .iter()
        .position(|object| object.fighter.is_some() && object.pos() == (x, y));

    match target_id {
        Some(target_id) => {
            let (player, target) = mut_two(GameConstants::PLAYER, target_id, objects);
            player.attack(target, &mut game);
        }
        None => move_by(GameConstants::PLAYER, dx, dy, &mut game, objects),
    }
}

fn pick_item_up(object_id: usize, objects: &mut Vec<GameObject>, game: &mut Game) {
    if game.inventory.len() >= 26 {
        game.log.add(
            format!(
                "Your inventory is full, cannot pick up {}",
                objects[object_id].name
            ),
            colors::RED,
        );
    } else {
        let item = objects.swap_remove(object_id);

        game.log
            .add(format!("You picked up a {}!", item.name), colors::GREEN);

        let index = game.inventory.len();
        let slot = item.equipment.map(|e| e.slot);
        game.inventory.push(item);

        // Auto-equip if slot is open
        if let Some(slot) = slot {
            if get_equipped_in_slot(slot, game).is_none() {
                game.inventory[index].equip(&mut game.log);
            }
        }
    }
}

fn ai_take_turn(
    monster_id: usize,
    objects: &mut [GameObject],
    mut tcod: &mut Tcod,
    mut game: &mut Game,
) {
    use Ai::*;

    if let Some(ai) = objects[monster_id].ai.take() {
        let new_ai = match ai {
            Basic => ai_basic(monster_id, objects, &mut tcod, &mut game),
            Confused {
                previous_ai,
                num_turns,
            } => ai_confused(monster_id, objects, &mut game, previous_ai, num_turns),
        };

        objects[monster_id].ai = Some(new_ai)
    }
}

fn ai_basic(
    monster_id: usize,
    objects: &mut [GameObject],
    tcod: &mut Tcod,
    mut game: &mut Game,
) -> Ai {
    // a basic monster takes its turn. If you can see it, it can see you.
    let (monster_x, monster_y) = objects[monster_id].pos();
    if tcod.fov.is_in_fov(monster_x, monster_y) {
        if objects[monster_id].distance_to(&objects[GameConstants::PLAYER]) >= 2.0 {
            let (player_x, player_y) = objects[GameConstants::PLAYER].pos();
            move_towards(monster_id, player_x, player_y, &mut game, objects);
        } else if objects[GameConstants::PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            let (monster, player) = mut_two(monster_id, GameConstants::PLAYER, objects);
            monster.attack(player, &mut game);
        }
    }

    Ai::Basic
}

fn ai_confused(
    monster_id: usize,
    objects: &mut [GameObject],
    mut game: &mut Game,
    previous_ai: Box<Ai>,
    num_turns: i32,
) -> Ai {
    if num_turns >= 0 {
        // still confused, move in a random direction and decrease status duration
        move_by(
            monster_id,
            rand::thread_rng().gen_range(-1, 2),
            rand::thread_rng().gen_range(-1, 2),
            &mut game,
            objects,
        );
        Ai::Confused {
            previous_ai,
            num_turns: num_turns - 1,
        }
    } else {
        // restore previous AI as this one gets cleared
        game.log.add(
            format!("The {} is no longer confused!", objects[monster_id].name),
            colors::RED,
        );
        *previous_ai
    }
}

fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32, tcod: &mut Tcod) -> Option<usize> {
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

fn inventory_menu(game: &Game, header: &str, tcod: &mut Tcod) -> Option<usize> {
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

fn use_item(inventory_id: usize, objects: &mut [GameObject], tcod: &mut Tcod, game: &mut Game) {
    use Item::*;

    // just call the "use_function" if it is defined
    if let Some(item) = game.inventory[inventory_id].item {
        let on_use = match item {
            Heal => spell_effects::heal::cast_heal,
            Lightning => spell_effects::lightning::cast_lightning,
            Confuse => spell_effects::confuse::cast_confuse,
            Fireball => spell_effects::fireball::cast_fireball,
            Sword => toggle_equipment,
            Shield => toggle_equipment,
        };

        match on_use(inventory_id, objects, game, tcod) {
            items::UseResult::UsedUp => {
                // destroy after use, unless it was cancelled for some reason
                game.inventory.remove(inventory_id);
            }
            items::UseResult::UsedAndKept => {} // do nothing
            items::UseResult::Cancelled => {
                game.log.add("Cancelled", colors::WHITE);
            }
        }
    } else {
        game.log.add(
            format!("The {} cannot be used.", game.inventory[inventory_id].name),
            colors::WHITE,
        );
    }
}

fn drop_item(inventory_id: usize, game: &mut Game, objects: &mut Vec<GameObject>) {
    let mut item = game.inventory.remove(inventory_id);

    if item.equipment.is_some() {
        item.dequip(&mut game.log);
    }

    item.set_pos(objects[GameConstants::PLAYER].x, objects[GameConstants::PLAYER].y);

    game.log
        .add(format!("You dropped a {}", item.name), colors::YELLOW);

    objects.push(item);
}

fn toggle_equipment(
    inventory_id: usize,
    _objects: &mut [GameObject],
    game: &mut Game,
    _tcod: &mut Tcod,
) -> items::UseResult {
    let equipment = match game.inventory[inventory_id].equipment {
        Some(equipment) => equipment,
        None => return items::UseResult::Cancelled,
    };

    if equipment.equipped {
        game.inventory[inventory_id].dequip(&mut game.log);
    } else {
        if let Some(old_equipment) = get_equipped_in_slot(equipment.slot, game) {
            game.inventory[old_equipment].dequip(&mut game.log);
        }

        game.inventory[inventory_id].equip(&mut game.log);
    }

    items::UseResult::UsedAndKept
}

fn get_equipped_in_slot(slot: Slot, game: &Game) -> Option<usize> {
    for (inventory_id, item) in game.inventory.iter().enumerate() {
        if item
            .equipment
            .as_ref()
            .map_or(false, |e| e.equipped && e.slot == slot)
        {
            return Some(inventory_id);
        }
    }
    None
}

/// Advance to the next level
fn next_level(tcod: &mut Tcod, objects: &mut Vec<GameObject>, game: &mut Game) {
    use GuiConstants::menus::next_level;

    game.log
        .add(next_level::REST_LOG_MESSAGE, next_level::REST_COLOR);
    let player = &mut objects[GameConstants::PLAYER];
    let heal_hp = player.max_hp(game) / 2;
    player.heal(heal_hp, game);

    game.log.add(
        next_level::NEXT_LEVEL_LOG_MESSAGE,
        next_level::NEXT_LEVEL_COLOR,
    );
    game.dungeon_level += 1;
    game.map = create_map(objects, game.dungeon_level);
    initialize_fov(game, tcod);
}

fn level_up(objects: &mut [GameObject], game: &mut Game, mut tcod: &mut Tcod) {
    use GuiConstants::menus::level_up;

    let player = &mut objects[GameConstants::PLAYER];
    let level_up_xp = GameConstants::LEVEL_UP_BASE + player.level * GameConstants::LEVEL_UP_FACTOR;

    // see if the player has enough xp
    if player.fighter.as_ref().map_or(0, |f| f.xp) >= level_up_xp {
        // level up!
        player.level += 1;
        game.log
            .add(level_up::create_log_message(player.level), colors::YELLOW);

        let mut fighter = player.fighter.as_mut().unwrap();
        let mut choice = None;

        while choice.is_none() {
            choice = menu(
                level_up::TITLE,
                &[
                    level_up::create_constitution_option(fighter.base_max_hp),
                    level_up::create_stength_option(fighter.base_power),
                    level_up::create_agility_option(fighter.base_defense),
                ],
                level_up::WIDTH,
                &mut tcod,
            );
        }

        fighter.xp -= level_up_xp;
        match choice {
            Some(0) => {
                fighter.base_max_hp += 20;
                fighter.hp += 20;
            }
            Some(1) => {
                fighter.base_power += 1;
            }
            Some(2) => {
                fighter.base_defense += 1;
            }
            _ => unreachable!(),
        }
    }
}

fn new_game(tcod: &mut Tcod) -> (Vec<GameObject>, Game) {
    use constants::player_base;
    let mut player = GameObject::new(
        0,
        0,
        player_base::SYMBOL,
        player_base::NAME,
        player_base::COLOR,
        true,
    );
    player.alive = true;
    player.fighter = Some(Fighter {
        base_max_hp: 100,
        hp: 100,
        base_defense: 1,
        base_power: 2,
        on_death: DeathCallback::Player,
        xp: 0,
    });

    let level = 1;
    let mut game_objects = vec![player];
    let mut game = Game {
        map: create_map(&mut game_objects, level),
        log: vec![],
        inventory: vec![],
        dungeon_level: 1,
    };

    use constants::gear::*;
    let mut dagger = GameObject::new(0, 0, dagger::SYMBOL, dagger::NAME, dagger::COLOR, false);
    dagger.item = Some(Item::Sword);
    dagger.equipment = Some(Equipment {
        equipped: true,
        slot: Slot::LeftHand,
        hp_bonus: dagger::HP_BONUS,
        defense_bonus: dagger::DEFENSE_BONUS,
        power_bonus: dagger::POWER_BONUS,
    });
    game.inventory.push(dagger);

    initialize_fov(&game, tcod);

    game.log.add(GuiConstants::WELCOME_MESSAGE, colors::RED);

    (game_objects, game)
}

fn initialize_fov(game: &Game, tcod: &mut Tcod) {
    for y in 0..GuiConstants::MAP_HEIGHT {
        for x in 0..GuiConstants::MAP_WIDTH {
            tcod.fov.set(
                x,
                y,
                !game.map[x as usize][y as usize].block_sight,
                !game.map[x as usize][y as usize].blocked,
            );
        }
    }

    tcod.con.clear(); // Ensure there is no carry over when returning to main menu and starting a new game
}

fn play_game(mut game_objects: Vec<GameObject>, mut game: &mut Game, mut tcod: &mut Tcod) {
    let mut key = Default::default();

    while !tcod.root.window_closed() {
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => tcod.mouse = m,
            Some((_, Event::Key(k))) => key = k,
            _ => key = Default::default(),
        }

        render::render_all(&mut tcod, &game_objects, &mut game);

        // Clear the GameObjects once their position is moved to the visible screen.
        // If we do this earlier or later we won't erase the last pos.
        for object in &game_objects {
            object.clear(&mut tcod.con);
        }

        // Handle player movement
        let action = handle_keys(key, &mut tcod, &mut game, &mut game_objects);

        if action == PlayerAction::Exit {
            save_game(&game_objects, game).unwrap();
            break;
        }

        if game_objects[GameConstants::PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..game_objects.len() {
                if game_objects[id].ai.is_some() {
                    ai_take_turn(id, &mut game_objects, &mut tcod, &mut game);
                }
            }
        }

        level_up(&mut game_objects, game, tcod);
    }
}

fn main_menu(mut tcod: &mut Tcod) {
    use GuiConstants::menus::*;
    let img = tcod::image::Image::from_file(main::IMAGE_PATH)
        .ok()
        .expect("Background image not found");

    while !tcod.root.window_closed() {
        // show the image, at twice the regular console resolution
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        tcod.root.set_default_foreground(colors::LIGHT_YELLOW);
        tcod.root.print_ex(
            GuiConstants::SCREEN_WIDTH / 2,
            GuiConstants::SCREEN_HEIGHT / 2 - 4,
            BackgroundFlag::None,
            TextAlignment::Center,
            constants::GAME_TITLE,
        );
        tcod.root.print_ex(
            GuiConstants::SCREEN_WIDTH / 2,
            GuiConstants::SCREEN_HEIGHT - 2,
            BackgroundFlag::None,
            TextAlignment::Center,
            main::AUTHOR_LINE,
        );

        // show options and wait for the players choice
        let choices = &[main::NEW_GAME, main::CONTINUE, main::QUIT];
        let choice = menu(
            main::MENU_NO_HEADER,
            choices,
            main::START_MENU_WIDTH,
            &mut tcod,
        );

        match choice {
            Some(0) => {
                // new game
                let (objects, mut game) = new_game(tcod);
                play_game(objects, &mut game, tcod);
            }
            Some(1) => match load_game() {
                Ok((objects, mut game)) => {
                    initialize_fov(&game, tcod);
                    play_game(objects, &mut game, tcod);
                }
                Err(_e) => {
                    msgbox("\nNo saved game to load.\n", 24, &mut tcod);
                    continue;
                }
            },
            Some(2) => {
                // quit
                break;
            }
            _ => {}
        }
    }
}

fn msgbox(text: &str, width: i32, mut tcod: &mut Tcod) {
    let options: &[&str] = &[];
    menu(text, options, width, &mut tcod);
}

fn save_game(objects: &[GameObject], game: &Game) -> Result<(), Box<dyn Error>> {
    let save_data = serde_json::to_string(&(objects, game))?;
    let mut file = File::create(constants::SAVE_FILE_NAME)?;
    file.write_all(save_data.as_bytes())?;
    Ok(())
}

fn load_game() -> Result<(Vec<GameObject>, Game), Box<dyn Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open(constants::SAVE_FILE_NAME)?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(Vec<GameObject>, Game)>(&json_save_state)?;
    Ok(result)
}

fn main() {
    let root = Root::initializer()
        .font(constants::FONT_PATH, FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(GuiConstants::SCREEN_WIDTH, GuiConstants::SCREEN_HEIGHT)
        .title(GuiConstants::menus::main::GAME_CONSOLE_HEADER)
        .init();

    tcod::system::set_fps(GameConstants::LIMIT_FPS);

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(GuiConstants::MAP_WIDTH, GuiConstants::MAP_HEIGHT),
        panel: Offscreen::new(GuiConstants::SCREEN_WIDTH, GuiConstants::PANEL_HEIGHT),
        fov: FovMap::new(GuiConstants::MAP_WIDTH, GuiConstants::MAP_HEIGHT),
        mouse: Default::default(),
    };

    main_menu(&mut tcod);
}
