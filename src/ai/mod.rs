use rand::Rng;
use tcod::colors;

use crate::game_objects::{Game, GameObject, MessageLog};
use crate::map;
use crate::data_manipulation::mut_two;

use crate::tcod_container::Tcod as Tcod;
use crate::constants::game as GameConstants;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Ai {
    Basic,
    Confused {
        previous_ai: Box<Ai>,
        num_turns: i32,
    },
}

pub fn take_turn(
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
        } else if objects[GameConstants::PLAYER]
            .fighter
            .map_or(false, |f| f.hp > 0)
        {
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

pub fn move_by(id: usize, dx: i32, dy: i32, game: &mut Game, objects: &mut [GameObject]) {
    let (x, y) = objects[id].pos();

    if !map::is_blocked(x + dx, y + dy, &game.map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}