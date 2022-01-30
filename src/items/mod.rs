use tcod::colors;

use crate::game_objects::{Game, GameObject, MessageLog, Slot};
use crate::items;
use crate::spell_effects;

use crate::tcod_container::Tcod as Tcod;
use crate::constants::game as GameConstants;

pub enum UseResult {
    UsedUp,
    UsedAndKept,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Item {
    Heal,
    Lightning,
    Confuse,
    Fireball,
    Sword,
    Shield,
}

pub fn pick_item_up(object_id: usize, objects: &mut Vec<GameObject>, game: &mut Game) {
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

pub fn toggle_equipment(
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

pub fn use_item(inventory_id: usize, objects: &mut [GameObject], tcod: &mut Tcod, game: &mut Game) {

    // just call the "use_function" if it is defined
    if let Some(item) = game.inventory[inventory_id].item {
        let on_use = match item {
            items::Item::Heal => spell_effects::heal::cast_heal,
            items::Item::Lightning => spell_effects::lightning::cast_lightning,
            items::Item::Confuse => spell_effects::confuse::cast_confuse,
            items::Item::Fireball => spell_effects::fireball::cast_fireball,
            items::Item::Sword => items::toggle_equipment,
            items::Item::Shield => items::toggle_equipment,
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

pub fn drop_item(inventory_id: usize, game: &mut Game, objects: &mut Vec<GameObject>) {
    let mut item = game.inventory.remove(inventory_id);

    if item.equipment.is_some() {
        item.dequip(&mut game.log);
    }

    item.set_pos(
        objects[GameConstants::PLAYER].x,
        objects[GameConstants::PLAYER].y,
    );

    game.log
        .add(format!("You dropped a {}", item.name), colors::YELLOW);

    objects.push(item);
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