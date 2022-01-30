use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use rand::Rng;
use std::cmp;

use tcod::colors;
use tcod::input::KeyCode::*;
use tcod::input::{self, Event};

use crate::constants;
use crate::enemies;
use crate::game_objects::{
    Ai, DeathCallback, Equipment, Fighter, Game, GameObject, Map, Slot, Tile,
};
use crate::items::Item;
use crate::render;
use crate::tcod_container;

use constants::game as GameConstants;
use constants::gui as GuiConstants;
use enemies::Enemies;
use tcod_container::Tcod;

#[derive(Clone, Copy, Debug)]
struct Rect {
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Rect {
            x1: x,
            y1: y,
            x2: x + w,
            y2: y + h,
        }
    }

    pub fn center(&self) -> (i32, i32) {
        let center_x = (self.x1 + self.x2) / 2;
        let center_y = (self.y1 + self.y2) / 2;

        (center_x, center_y)
    }

    pub fn intersects_with(&self, other: &Rect) -> bool {
        (self.x1 <= other.x2)
            && (self.x2 >= other.x1)
            && (self.y1 <= other.y2)
            && (self.y2 >= other.y1)
    }
}

struct Transition {
    level: u32,
    value: u32,
}

impl Transition {
    pub fn new(level: u32, value: u32) -> Self {
        Transition { level, value }
    }
}

fn create_room(room: Rect, map: &mut Map) {
    // These ranges need to be exclusive on both sides, so x+1..x works just fine
    for x in (room.x1 + 1)..room.x2 {
        for y in (room.y1 + 1)..room.y2 {
            map[x as usize][y as usize] = Tile::empty();
        }
    }
}

fn create_h_tunnel(x1: i32, x2: i32, y: i32, map: &mut Map) {
    for x in cmp::min(x1, x2)..=cmp::max(x1, x2) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

fn create_v_tunnel(y1: i32, y2: i32, x: i32, map: &mut Map) {
    for y in cmp::min(y1, y2)..=cmp::max(y1, y2) {
        map[x as usize][y as usize] = Tile::empty();
    }
}

/// Returns a value that depends on current dungeon level. The table specifies what
/// value occurs at each level, the default is 0
fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| level >= transition.level)
        .map_or(0, |transition| transition.value)
}

pub fn is_blocked(x: i32, y: i32, map: &Map, objects: &[GameObject]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

pub fn closest_monster(max_range: i32, objects: &mut [GameObject], tcod: &Tcod) -> Option<usize> {
    let mut closest_enemy = None;
    let mut closest_dist = (max_range + 1) as f32;

    for (id, object) in objects.iter().enumerate() {
        if (id != GameConstants::PLAYER)
            && object.fighter.is_some()
            && object.ai.is_some()
            && tcod.fov.is_in_fov(object.x, object.y)
        {
            let dist = objects[GameConstants::PLAYER].distance_to(object);
            if dist < closest_dist {
                closest_enemy = Some(id);
                closest_dist = dist;
            }
        }
    }

    closest_enemy
}

pub fn target_monster(
    tcod: &mut Tcod,
    objects: &[GameObject],
    game: &mut Game,
    max_range: Option<f32>,
) -> Option<usize> {
    loop {
        match target_tile(tcod, objects, game, max_range) {
            Some((x, y)) => {
                // return the first clicked monster, otherwise continue looping
                for (id, obj) in objects.iter().enumerate() {
                    if obj.pos() == (x, y) && obj.fighter.is_some() && id != GameConstants::PLAYER {
                        return Some(id);
                    }
                }
            }
            None => return None,
        }
    }
}

/// return the position of a tile left-clicked in player's FOV (optionally in a
/// range), or (None,None) if right-clicked.
pub fn target_tile(
    mut tcod: &mut Tcod,
    objects: &[GameObject],
    mut game: &mut Game,
    max_range: Option<f32>,
) -> Option<(i32, i32)> {
    loop {
        // render the screen. This erases the inventory and shows the names opf objects under the mouse.
        tcod.root.flush();
        let event = input::check_for_event(input::KEY_PRESS | input::MOUSE).map(|e| e.1);
        let mut key = None;
        match event {
            Some(Event::Mouse(m)) => tcod.mouse = m,
            Some(Event::Key(k)) => key = Some(k),
            None => {}
        }

        render::render_all(&mut tcod, objects, &mut game);

        let (x, y) = (tcod.mouse.cx as i32, tcod.mouse.cy as i32);

        // accept the target if the played clicked in FOV and in case a range is specified, if it's in that range
        let in_fov = (x < GuiConstants::MAP_WIDTH)
            && (y < GuiConstants::MAP_HEIGHT)
            && tcod.fov.is_in_fov(x, y);
        let in_range = max_range.map_or(true, |range| {
            objects[GameConstants::PLAYER].distance(x, y) <= range
        });

        if tcod.mouse.lbutton_pressed && in_fov && in_range {
            return Some((x, y));
        }

        let escape = key.map_or(false, |k| k.code == Escape);
        if tcod.mouse.rbutton_pressed || escape {
            return None;
        }
    }
}

pub fn create_map(objects: &mut Vec<GameObject>, level: u32) -> Map {
    let mut map = vec![
        vec![Tile::wall(); GuiConstants::MAP_HEIGHT as usize];
        GuiConstants::MAP_WIDTH as usize
    ];
    let mut rooms = vec![];

    // Player is the first element, remove everything else.
    // NOTE: works only when the player is the first object!
    assert_eq!(
        &objects[GameConstants::PLAYER] as *const _,
        &objects[0] as *const _
    );
    objects.truncate(1);

    for _ in 0..GameConstants::MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(
            GameConstants::ROOM_MIN_SIZE,
            GameConstants::ROOM_MAX_SIZE + 1,
        );
        let h = rand::thread_rng().gen_range(
            GameConstants::ROOM_MIN_SIZE,
            GameConstants::ROOM_MAX_SIZE + 1,
        );

        let x = rand::thread_rng().gen_range(0, GuiConstants::MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, GuiConstants::MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);
        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            // There are no intersections so we can process this
            create_room(new_room, &mut map);
            place_objects(new_room, &map, objects, level);

            let (center_x, center_y) = new_room.center();

            if rooms.is_empty() {
                objects[GameConstants::PLAYER].set_pos(center_x, center_y)
            } else {
                let (prev_x, prev_y) = rooms[rooms.len() - 1].center();

                if rand::random() {
                    create_h_tunnel(prev_x, center_x, prev_y, &mut map);
                    create_v_tunnel(prev_y, center_y, center_x, &mut map);
                } else {
                    create_v_tunnel(prev_y, center_y, prev_x, &mut map);
                    create_h_tunnel(prev_x, center_x, center_y, &mut map);
                }
            }

            rooms.push(new_room);
        }
    }

    let (last_room_x, last_room_y) = rooms[rooms.len() - 1].center();
    let mut stairs = GameObject::new(
        last_room_x,
        last_room_y,
        '<',
        "stairs",
        colors::WHITE,
        false,
    );

    stairs.always_visible = true;
    objects.push(stairs);

    map
}

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<GameObject>, level: u32) {
    let max_monsters = from_dungeon_level(
        &[
            Transition::new(1, 2),
            Transition::new(4, 3),
            Transition::new(6, 5),
        ],
        level,
    );

    let troll_chance = from_dungeon_level(
        &[
            Transition::new(3, 15),
            Transition::new(5, 30),
            Transition::new(7, 60),
        ],
        level,
    );

    let num_monsters = rand::thread_rng().gen_range(0, max_monsters + 1);

    for _ in 0..num_monsters {
        // Choose Random spot
        let mut x: i32;
        let mut y: i32;
        loop {
            x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
            y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

            if !objects.iter().any(|item| item.x == x && item.y == y) {
                break;
            };
        }

        let monster_chances = &mut [
            Weighted {
                weight: 80,
                item: Enemies::Orc,
            },
            Weighted {
                weight: troll_chance,
                item: Enemies::Troll,
            },
        ];

        let monster_choice = WeightedChoice::new(monster_chances);

        let mut monster = match monster_choice.ind_sample(&mut rand::thread_rng()) {
            Enemies::Orc => {
                let mut orc = GameObject::new(x, y, 'o', "Orc", colors::DESATURATED_GREEN, true);
                orc.fighter = Some(Fighter {
                    base_max_hp: 20,
                    hp: 20,
                    base_defense: 0,
                    base_power: 4,
                    on_death: DeathCallback::Monster,
                    xp: 35,
                });
                orc.ai = Some(Ai::Basic);
                orc
            }
            Enemies::Troll => {
                let mut troll = GameObject::new(x, y, 'T', "Troll", colors::DARKER_GREEN, true);
                troll.fighter = Some(Fighter {
                    base_max_hp: 30,
                    hp: 30,
                    base_defense: 2,
                    base_power: 8,
                    on_death: DeathCallback::Monster,
                    xp: 100,
                });
                troll.ai = Some(Ai::Basic);
                troll
            }
        };

        monster.alive = true;
        objects.push(monster);
    }

    let max_items = from_dungeon_level(&[Transition::new(1, 1), Transition::new(4, 2)], level);

    let item_chances = &mut [
        Weighted {
            weight: 35,
            item: Item::Heal,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition::new(4, 25)], level),
            item: Item::Lightning,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition::new(6, 25)], level),
            item: Item::Fireball,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition::new(2, 10)], level),
            item: Item::Confuse,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition::new(4, 5)], level),
            item: Item::Sword,
        },
        Weighted {
            weight: from_dungeon_level(&[Transition::new(8, 15)], level),
            item: Item::Shield,
        },
    ];

    let num_items = rand::thread_rng().gen_range(0, max_items + 1);

    for _ in 0..num_items {
        // choose random spot for this item
        let mut x: i32;
        let mut y: i32;
        loop {
            x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
            y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

            if !objects.iter().any(|item| item.x == x && item.y == y) {
                break;
            }
        }

        let item_choice = WeightedChoice::new(item_chances);

        if !is_blocked(x, y, map, objects) {
            let mut item = match item_choice.ind_sample(&mut rand::thread_rng()) {
                Item::Heal => {
                    let mut object =
                        GameObject::new(x, y, '!', "Healing Potion", colors::VIOLET, false);
                    object.item = Some(Item::Heal);
                    object
                }
                Item::Lightning => {
                    let mut object = GameObject::new(
                        x,
                        y,
                        '#',
                        "Scroll of Lightning Bolt",
                        colors::LIGHT_YELLOW,
                        false,
                    );
                    object.item = Some(Item::Lightning);
                    object
                }
                Item::Fireball => {
                    let mut object = GameObject::new(
                        x,
                        y,
                        '#',
                        "Scroll of Fireball",
                        colors::LIGHT_YELLOW,
                        false,
                    );
                    object.item = Some(Item::Fireball);
                    object
                }
                Item::Confuse => {
                    let mut object = GameObject::new(
                        x,
                        y,
                        '#',
                        "Scroll of Confusion",
                        colors::LIGHT_YELLOW,
                        false,
                    );
                    object.item = Some(Item::Confuse);
                    object
                }
                Item::Sword => {
                    let mut object = GameObject::new(x, y, '/', "Sword", colors::SKY, false);
                    object.item = Some(Item::Sword);
                    object.equipment = Some(Equipment {
                        equipped: false,
                        slot: Slot::RightHand,
                        power_bonus: 3,
                        defense_bonus: 0,
                        hp_bonus: 0,
                    });
                    object
                }
                Item::Shield => {
                    let mut object =
                        GameObject::new(x, y, '[', "Shield", colors::DARKER_ORANGE, false);
                    object.item = Some(Item::Shield);
                    object.equipment = Some(Equipment {
                        equipped: false,
                        slot: Slot::LeftHand,
                        hp_bonus: 0,
                        defense_bonus: 1,
                        power_bonus: 0,
                    });
                    object
                }
            };
            item.always_visible = true;
            objects.push(item);
        }
    }
}
