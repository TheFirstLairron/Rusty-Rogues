use tcod::colors;

use crate::constants::game as GameConstants;
use crate::game_objects::{Game, GameObject, MessageLog};
use crate::items::UseResult;
use crate::tcod_container;

use tcod_container::Tcod;

pub fn cast_heal(
    _inventory_id: usize,
    objects: &mut [GameObject],
    game: &mut Game,
    _tcod: &mut Tcod,
) -> UseResult {
    // heal the player
    let player = &mut objects[GameConstants::PLAYER];
    if let Some(fighter) = player.fighter {
        if fighter.hp == player.max_hp(game) {
            game.log.add("You are already at full health.", colors::RED);
            return UseResult::Cancelled;
        }

        game.log
            .add("Your wounds start to close up!", colors::LIGHT_VIOLET);
        objects[GameConstants::PLAYER].heal(GameConstants::HEAL_AMOUNT, game);
        return UseResult::UsedUp;
    }

    UseResult::Cancelled
}
