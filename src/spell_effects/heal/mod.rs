use tcod::colors;

use crate::tcod_container;
use crate::game_objects::{ GameObject, Game, MessageLog };
use crate::constants::game as GameConstants;
use crate::items;

use tcod_container::Tcod as Tcod;


pub fn cast_heal(
    _inventory_id: usize,
    objects: &mut [GameObject],
    game: &mut Game,
    _tcod: &mut Tcod,
) -> items::UseResult {
    // heal the player
    let player = &mut objects[GameConstants::PLAYER];
    if let Some(fighter) = player.fighter {
        if fighter.hp == player.max_hp(game) {
            game.log.add("You are already at full health.", colors::RED);
            return items::UseResult::Cancelled;
        }

        game.log
            .add("Your wounds start to close up!", colors::LIGHT_VIOLET);
        objects[GameConstants::PLAYER].heal(GameConstants::HEAL_AMOUNT, game);
        return items::UseResult::UsedUp;
    }

    items::UseResult::Cancelled
}