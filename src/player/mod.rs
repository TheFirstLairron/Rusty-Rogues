use tcod::colors;

use crate::ai;
use crate::game_objects::{Game, GameObject, MessageLog};
use crate::data_manipulation::mut_two;
use crate::tcod_container::Tcod;
use crate::render;

use crate::constants::game as GameConstants;
use crate::constants::gui as GuiConstants;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlayerAction {
    TookTurn,
    DidntTakeTurn,
    Exit,
}

pub fn move_or_attack(dx: i32, dy: i32, mut game: &mut Game, objects: &mut [GameObject]) {
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

pub fn try_level_up(objects: &mut [GameObject], game: &mut Game, mut tcod: &mut Tcod) {
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