use tcod::colors;

use crate::constants::game as GameConstants;
use crate::game_objects::{Game, GameObject, MessageLog};
use crate::items::UseResult;
use crate::map;
use crate::tcod_container;

use tcod_container::Tcod;

pub fn cast_lightning(
    _inventory_id: usize,
    objects: &mut [GameObject],
    mut game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    // find the closest enemy inside a max range and damage it
    let monster_id = map::closest_monster(GameConstants::LIGHTNING_RANGE, objects, &tcod);
    if let Some(monster_id) = monster_id {
        // ZAP
        game.log.add(format!("A lightning bolt strikes the {} with a loud thunder! \n The damage is {} hit points ", objects[monster_id].name, GameConstants::LIGHTNING_DAMAGE), colors::LIGHT_BLUE);

        if let Some(xp) =
            objects[monster_id].take_damage(GameConstants::LIGHTNING_DAMAGE, &mut game)
        {
            objects[GameConstants::PLAYER].fighter.as_mut().unwrap().xp += xp;
        };

        UseResult::UsedUp
    } else {
        // No enemy found within max range
        game.log
            .add("No enemy is close enough to strike.", colors::RED);
        UseResult::Cancelled
    }
}
