use tcod::colors;

use crate::tcod_container;
use crate::game_objects::{ GameObject, Game, MessageLog, Ai };
use crate::constants::game as GameConstants;
use crate::items::{ UseResult };
use crate::map;

use tcod_container::Tcod as Tcod;


pub fn cast_confuse(
    _inventory_id: usize,
    objects: &mut [GameObject],
    game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    // ask the player for a target to confuse
    game.log.add(
        "Left-click an enemy to confuse it, or right-click to cancel.",
        colors::LIGHT_CYAN,
    );

    let monster_id = map::target_monster(tcod, objects, game, Some(GameConstants::CONFUSE_RANGE as f32));

    if let Some(monster_id) = monster_id {
        let old_ai = objects[monster_id].ai.take().unwrap_or(Ai::Basic);
        objects[monster_id].ai = Some(Ai::Confused {
            previous_ai: Box::new(old_ai),
            num_turns: GameConstants::CONFUSE_NUM_TURNS,
        });

        game.log.add(
            format!(
                "The eyes of the {} look vacant, as it starts to stumble around!",
                objects[monster_id].name
            ),
            colors::LIGHT_GREEN,
        );

        UseResult::UsedUp
    } else {
        // No enemy found in range
        game.log
            .add("No enemy is close enough to strike.", colors::RED);
        UseResult::Cancelled
    }
}