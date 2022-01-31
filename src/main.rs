#[macro_use]
extern crate serde_derive;

extern crate rand;
extern crate tcod;

mod ai;
mod constants;
mod data_manipulation;
mod enemies;
mod game_objects;
mod items;
mod map;
mod render;
mod spell_effects;
mod tcod_container;
mod persistence;

use tcod::colors;
use tcod::console::*;
use tcod::input::Key;
use tcod::input::KeyCode::*;
use tcod::input::{self, Event};
use tcod::map::Map as FovMap;

use game_objects::{Game, GameObject, Fighter, Equipment, Slot, DeathCallback, MessageLog};
use tcod_container::Tcod;
use data_manipulation::mut_two;
use map::create_map;

use constants::game as GameConstants;
use constants::gui as GuiConstants;


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
            let item_id = objects.iter().position(|object| {
                object.pos() == objects[GameConstants::PLAYER].pos() && object.item.is_some()
            });

            if let Some(item_id) = item_id {
                items::pick_item_up(item_id, objects, game);
            }

            DidntTakeTurn
        }
        (Key { printable: 'i', .. }, true) => {
            // show the inventory: if an item is selected, use it
            let inventory_index = render::inventory_menu(
                game,
                "Press the key next to an item to use it, or any other to cancel. \n",
                &mut tcod,
            );

            if let Some(inventory_index) = inventory_index {
                items::use_item(inventory_index, objects, tcod, game)
            }

            DidntTakeTurn
        }
        (Key { printable: 'd', .. }, true) => {
            // show the inventory; if an item is selected, drop it
            let inventory_index = render::inventory_menu(
                game,
                "Press the key next to an item to drop it, or any other to cancel.\n",
                &mut tcod,
            );
            if let Some(inventory_index) = inventory_index {
                items::drop_item(inventory_index, &mut game, objects);
            }
            DidntTakeTurn
        }
        (Key { printable: 'c', .. }, true) => {
            // show character information
            let player = &objects[GameConstants::PLAYER];
            let level = player.level;
            let level_up_xp =
                GameConstants::LEVEL_UP_BASE + player.level * GameConstants::LEVEL_UP_FACTOR;
            if let Some(fighter) = player.fighter.as_ref() {
                let msg = format!(
                    "Character Information: \n* Level: {} \n* Experience: {} \n* Experience to level up: {} \n\n* Maximum HP: {} \n* Attack: {} \n* Defense: {} \n",
                    level, fighter.xp, level_up_xp, player.max_hp(game), player.power(game), player.defense(game)
                );
                render::msgbox(&msg, GuiConstants::CHARACTER_SCREEN_WIDTH, &mut tcod);
            }

            DidntTakeTurn
        }
        (Key { printable: '<', .. }, true) => {
            // go down the stairs if the player is on them
            let player_on_stairs = objects.iter().any(|object| {
                object.pos() == objects[GameConstants::PLAYER].pos() && object.name == "stairs"
            });

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
        None => ai::move_by(GameConstants::PLAYER, dx, dy, &mut game, objects),
    }
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
            choice = render::menu(
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
    dagger.item = Some(items::Item::Sword);
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
            persistence::save_game(&game_objects, game).unwrap();
            break;
        }

        if game_objects[GameConstants::PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..game_objects.len() {
                if game_objects[id].ai.is_some() {
                    ai::take_turn(id, &mut game_objects, &mut tcod, &mut game);
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
        let choice = render::menu(
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
            Some(1) => match persistence::load_game() {
                Ok((objects, mut game)) => {
                    initialize_fov(&game, tcod);
                    play_game(objects, &mut game, tcod);
                }
                Err(_e) => {
                    render::msgbox("\nNo saved game to load.\n", 24, &mut tcod);
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
