use crate::tcod_container;
use crate::game_objects::{ GameObject, Game, MessageLog };
use crate::constants::game as GameConstants;
use crate::constants::consumables::scrolls::fireball as FireballConstants;
use crate::items;
use crate::map;

use tcod_container::Tcod as Tcod;

pub fn cast_fireball(
    _inventory_id: usize,
    objects: &mut [GameObject],
    mut game: &mut Game,
    tcod: &mut Tcod,
) -> items::UseResult {
    // Ask the player for a target tile to throw a fireball at
    game.log
        .add(FireballConstants::INSTRUCTIONS, FireballConstants::INSTRUCTION_COLOR);

    let (x, y) = match map::target_tile(tcod, objects, game, None) {
        Some(tile_pos) => tile_pos,
        None => return items::UseResult::Cancelled,
    };

    game.log
        .add(FireballConstants::create_radius_message(), FireballConstants::RADIUS_COLOR);

    let mut xp_to_gain = 0;
    for (id, obj) in objects.iter_mut().enumerate() {
        if obj.distance(x, y) <= FireballConstants::RADIUS as f32 && obj.fighter.is_some() {
            game.log.add(
                FireballConstants::create_damage_message(&obj.name),
                FireballConstants::DAMAGE_COLOR,
            );

            if let Some(xp) = obj.take_damage(FireballConstants::DAMAGE, &mut game) {
                // can't alter player in this loop, and don't wanna give them xp for killing themselves.
                // so we track it outside the loop and then award it after
                if id != GameConstants::PLAYER {
                    xp_to_gain = xp;
                }
            };
        }
    }

    objects[GameConstants::PLAYER].fighter.as_mut().unwrap().xp += xp_to_gain;

    items::UseResult::UsedUp
}