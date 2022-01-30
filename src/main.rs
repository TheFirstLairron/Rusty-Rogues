#[macro_use]
extern crate serde_derive;

extern crate rand;
extern crate tcod;

mod constants;

use tcod::colors::{self, Color};
use tcod::console::*;
use tcod::input::Key;
use tcod::input::KeyCode::*;
use tcod::input::{self, Event, Mouse};
use tcod::map::Map as FovMap;

use std::cmp;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use rand::distributions::{IndependentSample, Weighted, WeightedChoice};
use rand::Rng;

use constants::game as GameConstants;

type Map = Vec<Vec<Tile>>;
type Messages = Vec<(String, Color)>;

#[derive(Debug, Serialize, Deserialize)]
struct GameObject {
    x: i32,
    y: i32,
    char: char,
    color: Color,
    name: String,
    blocks: bool,
    alive: bool,
    fighter: Option<Fighter>,
    ai: Option<Ai>,
    item: Option<Item>,
    always_visible: bool,
    level: i32,
    equipment: Option<Equipment>,
}

impl GameObject {
    pub fn new(x: i32, y: i32, char: char, name: &str, color: Color, blocks: bool) -> Self {
        GameObject {
            x,
            y,
            char,
            color,
            name: name.into(),
            blocks,
            alive: false,
            fighter: None,
            ai: None,
            item: None,
            always_visible: false,
            level: 1,
            equipment: None,
        }
    }

    pub fn draw(&self, con: &mut dyn Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn clear(&self, con: &mut dyn Console) {
        con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn distance(&self, x: i32, y: i32) -> f32 {
        (((x - self.x).pow(2) + (y - self.y).pow(2)) as f32).sqrt()
    }

    pub fn distance_to(&self, other: &GameObject) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32, mut game: &mut Game) -> Option<i32> {
        // apply damage if possible
        if let Some(fighter) = self.fighter.as_mut() {
            if damage > 0 {
                fighter.hp -= damage;
            }
        }

        // check for death, call the death function
        if let Some(fighter) = self.fighter {
            if fighter.hp <= 0 {
                self.alive = false;
                fighter.on_death.callback(self, &mut game);
                return Some(fighter.xp);
            }
        }

        None
    }

    pub fn attack(&mut self, target: &mut GameObject, mut game: &mut Game) {
        // A simple formula for attack damage
        let damage = self.power(game) - target.defense(game);

        if damage > 0 {
            // Make the target take some damage
            game.log.add(
                format!(
                    "{} attacks {} for {} hit points",
                    self.name, target.name, damage
                ),
                colors::WHITE,
            );
            if let Some(xp) = target.take_damage(damage, &mut game) {
                // give xp to fighter. Only relevant if player, but no need to check.
                self.fighter.as_mut().unwrap().xp += xp;
            };
        } else {
            game.log.add(
                format!(
                    "{} attacks {} but it has no effect!",
                    self.name, target.name
                ),
                colors::WHITE,
            );
        }
    }

    pub fn heal(&mut self, amount: i32, game: &Game) {
        let max_hp = self.max_hp(game);
        if let Some(ref mut fighter) = self.fighter {
            fighter.hp += amount;

            if fighter.hp > max_hp {
                fighter.hp = max_hp;
            }
        }
    }

    pub fn equip(&mut self, log: &mut Messages) {
        if self.item.is_none() {
            log.add(
                format!("Can't equip {:?} because it's not an Item", self),
                colors::RED,
            );
            return;
        }

        if let Some(ref mut equipment) = self.equipment {
            if !equipment.equipped {
                equipment.equipped = true;
                log.add(
                    format!("Equipped {} on {}.", self.name, equipment.slot),
                    colors::LIGHT_GREEN,
                );
            }
        } else {
            log.add(
                format!("Can't equip {:?} because it's not an Equipment.", self),
                colors::RED,
            );
        }
    }

    pub fn dequip(&mut self, log: &mut Vec<(String, Color)>) {
        if self.item.is_none() {
            log.add(
                format!("Can't dequip {:?} because it's not an Item.", self),
                colors::RED,
            );
            return;
        };
        if let Some(ref mut equipment) = self.equipment {
            if equipment.equipped {
                equipment.equipped = false;
                log.add(
                    format!("Dequipped {} from {}.", self.name, equipment.slot),
                    colors::LIGHT_YELLOW,
                );
            }
        } else {
            log.add(
                format!("Can't dequip {:?} because it's not an Equipment.", self),
                colors::RED,
            );
        }
    }

    pub fn power(&self, game: &Game) -> i32 {
        let base_power = self.fighter.map_or(0, |f| f.base_power);
        let bonus_power: i32 = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.power_bonus)
            .sum();

        base_power + bonus_power
    }

    pub fn defense(&self, game: &Game) -> i32 {
        let base_defense = self.fighter.map_or(0, |f| f.base_defense);
        let bonus_defense: i32 = self
            .get_all_equipped(game)
            .iter()
            .map(|e| e.defense_bonus)
            .sum();

        base_defense + bonus_defense
    }

    pub fn max_hp(&self, game: &Game) -> i32 {
        let base_max_hp = self.fighter.map_or(0, |f| f.base_max_hp);
        let bonus_max_hp: i32 = self.get_all_equipped(game).iter().map(|e| e.hp_bonus).sum();

        base_max_hp + bonus_max_hp
    }

    pub fn get_all_equipped(&self, game: &Game) -> Vec<Equipment> {
        if self.name == constants::player_base::NAME {
            game.inventory
                .iter()
                .filter(|item| item.equipment.map_or(false, |e| e.equipped))
                .map(|item| item.equipment.unwrap())
                .collect()
        } else {
            vec![]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Fighter {
    base_max_hp: i32,
    hp: i32,
    base_defense: i32,
    base_power: i32,
    on_death: DeathCallback,
    xp: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut GameObject, mut game: &mut Game) {
        use DeathCallback::*;
        let callback: fn(&mut GameObject, &mut Game) = match self {
            Player => player_death,
            Monster => monster_death,
        };

        callback(object, &mut game);
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Ai {
    Basic,
    Confused {
        previous_ai: Box<Ai>,
        num_turns: i32,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Tile {
    blocked: bool,
    block_sight: bool,
    explored: bool,
}

impl Tile {
    pub fn empty() -> Self {
        Tile {
            blocked: false,
            block_sight: false,
            explored: false,
        }
    }

    pub fn wall() -> Self {
        Tile {
            blocked: true,
            block_sight: true,
            explored: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Item {
    Heal,
    Lightning,
    Confuse,
    Fireball,
    Sword,
    Shield,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
struct Equipment {
    slot: Slot,
    equipped: bool,
    power_bonus: i32,
    defense_bonus: i32,
    hp_bonus: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Slot {
    Head,
    RightHand,
    LeftHand,
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            Slot::LeftHand => write!(f, "left hand"),
            Slot::RightHand => write!(f, "right hand"),
            Slot::Head => write!(f, "head"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
enum Enemies {
    Orc,
    Troll,
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

enum UseResult {
    UsedUp,
    UsedAndKept,
    Cancelled,
}

struct Tcod {
    root: Root,
    con: Offscreen,
    panel: Offscreen,
    fov: FovMap,
    mouse: Mouse,
}

trait MessageLog {
    fn add<T: Into<String>>(&mut self, message: T, color: Color);
}

impl MessageLog for Vec<(String, Color)> {
    fn add<T: Into<String>>(&mut self, message: T, color: Color) {
        self.push((message.into(), color));
    }
}

#[derive(Serialize, Deserialize)]
struct Game {
    map: Map,
    log: Messages,
    inventory: Vec<GameObject>,
    dungeon_level: u32,
}

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
            let item_id = objects
                .iter()
                .position(|object| object.pos() == objects[GameConstants::PLAYER].pos() && object.item.is_some());

            if let Some(item_id) = item_id {
                pick_item_up(item_id, objects, game);
            }

            DidntTakeTurn
        }
        (Key { printable: 'i', .. }, true) => {
            // show the inventory: if an item is selected, use it
            let inventory_index = inventory_menu(
                game,
                "Press the key next to an item to use it, or any other to cancel. \n",
                &mut tcod,
            );

            if let Some(inventory_index) = inventory_index {
                use_item(inventory_index, objects, tcod, game)
            }

            DidntTakeTurn
        }
        (Key { printable: 'd', .. }, true) => {
            // show the inventory; if an item is selected, drop it
            let inventory_index = inventory_menu(
                game,
                "Press the key next to an item to drop it, or any other to cancel.\n",
                &mut tcod,
            );
            if let Some(inventory_index) = inventory_index {
                drop_item(inventory_index, &mut game, objects);
            }
            DidntTakeTurn
        }
        (Key { printable: 'c', .. }, true) => {
            // show character information
            let player = &objects[GameConstants::PLAYER];
            let level = player.level;
            let level_up_xp = GameConstants::LEVEL_UP_BASE + player.level * GameConstants::LEVEL_UP_FACTOR;
            if let Some(fighter) = player.fighter.as_ref() {
                let msg = format!(
                    "Character Information: \n* Level: {} \n* Experience: {} \n* Experience to level up: {} \n\n* Maximum HP: {} \n* Attack: {} \n* Defense: {} \n",
                    level, fighter.xp, level_up_xp, player.max_hp(game), player.power(game), player.defense(game)
                );
                msgbox(&msg, constants::gui::CHARACTER_SCREEN_WIDTH, &mut tcod);
            }

            DidntTakeTurn
        }
        (Key { printable: '<', .. }, true) => {
            // go down the stairs if the player is on them
            let player_on_stairs = objects
                .iter()
                .any(|object| object.pos() == objects[GameConstants::PLAYER].pos() && object.name == "stairs");

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

fn render_all(tcod: &mut Tcod, game_objects: &[GameObject], game: &mut Game) {
    // originally checked if user moved, but that caused a bug: every action was delayed by one turn. No observable adverse effects from removing the check.
    let player = &game_objects[GameConstants::PLAYER];
    tcod.fov
        .compute_fov(player.x, player.y, GameConstants::TORCH_RADIUS, GameConstants::FOV_LIGHT_WALLS, GameConstants::FOV_ALGO);

    // Go through all tiles and set their background color
    for y in 0..constants::gui::MAP_HEIGHT {
        for x in 0..constants::gui::MAP_WIDTH {
            // check if it's a wall by checking if it blocks sight
            let visible = tcod.fov.is_in_fov(x, y);
            let is_wall = game.map[x as usize][y as usize].block_sight;
            let color = match (visible, is_wall) {
                // Outside FOV
                (false, true) => GameConstants::COLOR_DARK_WALL,
                (false, false) => GameConstants::COLOR_DARK_GROUND,
                // Inside FOV
                (true, true) => GameConstants::COLOR_LIGHT_WALL,
                (true, false) => GameConstants::COLOR_LIGHT_GROUND,
            };

            let explored = &mut game.map[x as usize][y as usize].explored;

            if visible {
                *explored = true;
            }

            if *explored {
                tcod.con
                    .set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // Draw the GameObjects
    let mut to_draw: Vec<_> = game_objects
        .iter()
        .filter(|item| {
            tcod.fov.is_in_fov(item.x, item.y)
                || (item.always_visible && game.map[item.x as usize][item.y as usize].explored)
        })
        .collect();
    // Sort so that non-blocking objets come first
    to_draw.sort_by(|item1, item2| item1.blocks.cmp(&item2.blocks));
    // Draw the items in the list
    for object in to_draw {
        // only render if in FOV
        object.draw(&mut tcod.con);
    }

    // Blit onto the actual screen
    blit(
        &tcod.con,
        (0, 0),
        (constants::gui::SCREEN_WIDTH, constants::gui::SCREEN_HEIGHT),
        &mut tcod.root,
        (0, 0),
        1.0,
        1.0,
    );

    tcod.panel.set_default_background(colors::BLACK);
    tcod.panel.clear();

    // Print the game messages, one line at a time
    let mut y = constants::gui::MSG_HEIGHT as i32;

    for &(ref msg, color) in game.log.iter().rev() {
        let msg_height =
            tcod.panel
                .get_height_rect(constants::gui::MSG_X, y, constants::gui::MSG_WIDTH, 0, msg);
        y -= msg_height;

        if y < 0 {
            break;
        }

        tcod.panel.set_default_foreground(color);
        tcod.panel
            .print_rect(constants::gui::MSG_X, y, constants::gui::MSG_WIDTH, 0, msg);
    }

    // Show the players stats
    let player = &game_objects[GameConstants::PLAYER];
    let hp = player.fighter.map_or(0, |f| f.hp);
    let max_hp = player.max_hp(game);

    render_bar(
        &mut tcod.panel,
        1,
        1,
        constants::gui::BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        colors::LIGHT_RED,
        colors::DARKER_RED,
    );

    tcod.panel.print_ex(
        1,
        3,
        BackgroundFlag::None,
        TextAlignment::Left,
        format!("Dungeon Level: {}", game.dungeon_level),
    );

    // Display the names of the objects under th mouse
    tcod.panel.set_default_foreground(colors::LIGHT_GREY);
    tcod.panel.print_ex(
        1,
        0,
        BackgroundFlag::None,
        TextAlignment::Left,
        get_names_under_mouse(tcod.mouse, game_objects, &tcod.fov),
    );

    blit(
        &tcod.panel,
        (0, 0),
        (constants::gui::SCREEN_WIDTH, constants::gui::PANEL_HEIGHT),
        &mut tcod.root,
        (0, constants::gui::PANEL_Y),
        1.0,
        1.0,
    );

    // Make the console actually visible
    tcod.root.flush();
}

fn create_map(objects: &mut Vec<GameObject>, level: u32) -> Map {
    let mut map = vec![
        vec![Tile::wall(); constants::gui::MAP_HEIGHT as usize];
        constants::gui::MAP_WIDTH as usize
    ];
    let mut rooms = vec![];

    // Player is the first element, remove everything else.
    // NOTE: works only when the player is the first object!
    assert_eq!(&objects[GameConstants::PLAYER] as *const _, &objects[0] as *const _);
    objects.truncate(1);

    for _ in 0..GameConstants::MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(GameConstants::ROOM_MIN_SIZE, GameConstants::ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(GameConstants::ROOM_MIN_SIZE, GameConstants::ROOM_MAX_SIZE + 1);

        let x = rand::thread_rng().gen_range(0, constants::gui::MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, constants::gui::MAP_HEIGHT - h);

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

fn is_blocked(x: i32, y: i32, map: &Map, objects: &[GameObject]) -> bool {
    if map[x as usize][y as usize].blocked {
        return true;
    }

    objects
        .iter()
        .any(|object| object.blocks && object.pos() == (x, y))
}

fn move_by(id: usize, dx: i32, dy: i32, game: &mut Game, objects: &mut [GameObject]) {
    let (x, y) = objects[id].pos();

    if !is_blocked(x + dx, y + dy, &game.map, objects) {
        objects[id].set_pos(x + dx, y + dy);
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
        None => move_by(GameConstants::PLAYER, dx, dy, &mut game, objects),
    }
}

fn pick_item_up(object_id: usize, objects: &mut Vec<GameObject>, game: &mut Game) {
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

fn ai_take_turn(
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
        } else if objects[GameConstants::PLAYER].fighter.map_or(false, |f| f.hp > 0) {
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

fn mut_two<T>(first_index: usize, second_index: usize, items: &mut [T]) -> (&mut T, &mut T) {
    assert!(first_index != second_index);
    let split_at_index = cmp::max(first_index, second_index);
    let (first_slice, second_slice) = items.split_at_mut(split_at_index);
    if first_index < second_index {
        (&mut first_slice[first_index], &mut second_slice[0])
    } else {
        (&mut second_slice[0], &mut first_slice[second_index])
    }
}

fn player_death(player: &mut GameObject, game: &mut Game) {
    // The game ended!
    game.log.add("You died!", colors::RED);

    player.char = '%';
    player.color = colors::DARK_RED;
    player.name = "Corpse of player".to_string();
}

fn monster_death(monster: &mut GameObject, game: &mut Game) {
    // Transform into corpse. Won't block, can't attack/be attacked, and doesn't move
    game.log.add(
        format!(
            "{} is dead! You gain {} experience points.",
            monster.name,
            monster.fighter.unwrap().xp
        ),
        colors::ORANGE,
    );
    monster.char = '%';
    monster.color = colors::DARK_RED;
    monster.blocks = false;
    monster.fighter = None;
    monster.ai = None;
    monster.name = format!("Remains of {}", monster.name);
}

fn render_bar(
    panel: &mut Offscreen,
    x: i32,
    y: i32,
    total_width: i32,
    name: &str,
    value: i32,
    maximum: i32,
    bar_color: Color,
    back_color: Color,
) {
    // Calculate the width of the bar
    let bar_width = (value as f32 / maximum as f32 * total_width as f32) as i32;

    // Render the background
    panel.set_default_background(back_color);
    panel.rect(x, y, total_width, 1, false, BackgroundFlag::Screen);

    // Render the Bar
    panel.set_default_background(bar_color);
    if bar_width > 0 {
        panel.rect(x, y, bar_width, 1, false, BackgroundFlag::Screen);
    }

    panel.set_default_foreground(colors::WHITE);
    panel.print_ex(
        x + total_width / 2,
        y,
        BackgroundFlag::None,
        TextAlignment::Center,
        &format!("{}: {}/{}", name, value, maximum),
    );
}

fn get_names_under_mouse(mouse: Mouse, objects: &[GameObject], fov_map: &FovMap) -> String {
    let (x, y) = (mouse.cx as i32, mouse.cy as i32);

    let names = objects
        .iter()
        .filter(|obj| obj.pos() == (x, y) && fov_map.is_in_fov(obj.x, obj.y))
        .map(|obj| obj.name.clone())
        .collect::<Vec<_>>();

    names.join(", ")
}

fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32, tcod: &mut Tcod) -> Option<usize> {
    assert!(
        options.len() <= 26,
        "Cannot have a menu with more than 26 options"
    );

    // calculate total height for the header (after auto-wrap) and one line per option
    let header_height = if header.is_empty() {
        0
    } else {
        tcod.root
            .get_height_rect(0, 0, width, constants::gui::SCREEN_HEIGHT, header)
    };

    let height = options.len() as i32 + header_height;

    let mut window = Offscreen::new(width, height);

    // print the header, with auto-wrap;
    window.set_default_foreground(colors::WHITE);
    window.print_rect_ex(
        0,
        0,
        width,
        height,
        BackgroundFlag::None,
        TextAlignment::Left,
        header,
    );

    // print all the options
    for (index, option_text) in options.iter().enumerate() {
        // essentially ASCII math, probably a better way of approaching this entire menu
        let menu_letter = (b'a' + index as u8) as char;
        let text = format!("({}) {}", menu_letter, option_text.as_ref());
        window.print_ex(
            0,
            header_height + index as i32,
            BackgroundFlag::None,
            TextAlignment::Left,
            text,
        );
    }

    let x = constants::gui::SCREEN_WIDTH / 2 - width / 2;
    let y = constants::gui::SCREEN_HEIGHT / 2 - height / 2;
    tcod::console::blit(
        &window,
        (0, 0),
        (width, height),
        &mut tcod.root,
        (x, y),
        1.0,
        0.7,
    );

    // present the root console to the player and wait for a key press
    tcod.root.flush();
    let key = tcod.root.wait_for_keypress(true);

    // convert the ASCII code to an index; if it corresponds to an option, return it
    if key.printable.is_alphabetic() {
        let index = key.printable.to_ascii_lowercase() as usize - 'a' as usize;
        if index < options.len() {
            Some(index)
        } else {
            None
        }
    } else {
        None
    }
}

fn inventory_menu(game: &Game, header: &str, tcod: &mut Tcod) -> Option<usize> {
    let options = if game.inventory.is_empty() {
        vec!["Inventory is empty.".into()]
    } else {
        game.inventory
            .iter()
            .map(|item| match item.equipment {
                Some(equipment) if equipment.equipped => {
                    format!("{} (on {})", item.name, equipment.slot)
                }
                _ => item.name.clone(),
            })
            .collect()
    };

    let inventory_index = menu(header, &options, constants::gui::INVENTORY_WIDTH, tcod);

    // if an item was chosen, return it
    if !game.inventory.is_empty() {
        inventory_index
    } else {
        None
    }
}

fn use_item(inventory_id: usize, objects: &mut [GameObject], tcod: &mut Tcod, game: &mut Game) {
    use Item::*;

    // just call the "use_function" if it is defined
    if let Some(item) = game.inventory[inventory_id].item {
        let on_use = match item {
            Heal => cast_heal,
            Lightning => cast_lightning,
            Confuse => cast_confuse,
            Fireball => cast_fireball,
            Sword => toggle_equipment,
            Shield => toggle_equipment,
        };

        match on_use(inventory_id, objects, game, tcod) {
            UseResult::UsedUp => {
                // destroy after use, unless it was cancelled for some reason
                game.inventory.remove(inventory_id);
            }
            UseResult::UsedAndKept => {} // do nothing
            UseResult::Cancelled => {
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

fn drop_item(inventory_id: usize, game: &mut Game, objects: &mut Vec<GameObject>) {
    let mut item = game.inventory.remove(inventory_id);

    if item.equipment.is_some() {
        item.dequip(&mut game.log);
    }

    item.set_pos(objects[GameConstants::PLAYER].x, objects[GameConstants::PLAYER].y);

    game.log
        .add(format!("You dropped a {}", item.name), colors::YELLOW);

    objects.push(item);
}

fn closest_monster(max_range: i32, objects: &mut [GameObject], tcod: &Tcod) -> Option<usize> {
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

/// return the position of a tile left-clicked in player's FOV (optionally in a
/// range), or (None,None) if right-clicked.
fn target_tile(
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

        render_all(&mut tcod, objects, &mut game);

        let (x, y) = (tcod.mouse.cx as i32, tcod.mouse.cy as i32);

        // accept the target if the played clicked in FOV and in case a range is specified, if it's in that range
        let in_fov = (x < constants::gui::MAP_WIDTH)
            && (y < constants::gui::MAP_HEIGHT)
            && tcod.fov.is_in_fov(x, y);
        let in_range = max_range.map_or(true, |range| objects[GameConstants::PLAYER].distance(x, y) <= range);

        if tcod.mouse.lbutton_pressed && in_fov && in_range {
            return Some((x, y));
        }

        let escape = key.map_or(false, |k| k.code == Escape);
        if tcod.mouse.rbutton_pressed || escape {
            return None;
        }
    }
}

fn target_monster(
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

fn cast_heal(
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

fn cast_lightning(
    _inventory_id: usize,
    objects: &mut [GameObject],
    mut game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    // find the closest enemy inside a max range and damage it
    let monster_id = closest_monster(GameConstants::LIGHTNING_RANGE, objects, &tcod);
    if let Some(monster_id) = monster_id {
        // ZAP
        game.log.add(format!("A lightning bolt strikes the {} with a loud thunder! \n The damage is {} hit points ", objects[monster_id].name, GameConstants::LIGHTNING_DAMAGE), colors::LIGHT_BLUE);

        if let Some(xp) = objects[monster_id].take_damage(GameConstants::LIGHTNING_DAMAGE, &mut game) {
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

fn cast_confuse(
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

    let monster_id = target_monster(tcod, objects, game, Some(GameConstants::CONFUSE_RANGE as f32));

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

fn cast_fireball(
    _inventory_id: usize,
    objects: &mut [GameObject],
    mut game: &mut Game,
    tcod: &mut Tcod,
) -> UseResult {
    use constants::consumables::scrolls::fireball;
    // Ask the player for a target tile to throw a fireball at
    game.log
        .add(fireball::INSTRUCTIONS, fireball::INSTRUCTION_COLOR);

    let (x, y) = match target_tile(tcod, objects, game, None) {
        Some(tile_pos) => tile_pos,
        None => return UseResult::Cancelled,
    };

    game.log
        .add(fireball::create_radius_message(), fireball::RADIUS_COLOR);

    let mut xp_to_gain = 0;
    for (id, obj) in objects.iter_mut().enumerate() {
        if obj.distance(x, y) <= fireball::RADIUS as f32 && obj.fighter.is_some() {
            game.log.add(
                fireball::create_damage_message(&obj.name),
                fireball::DAMAGE_COLOR,
            );

            if let Some(xp) = obj.take_damage(fireball::DAMAGE, &mut game) {
                // can't alter player in this loop, and don't wanna give them xp for killing themselves.
                // so we track it outside the loop and then award it after
                if id != GameConstants::PLAYER {
                    xp_to_gain = xp;
                }
            };
        }
    }

    objects[GameConstants::PLAYER].fighter.as_mut().unwrap().xp += xp_to_gain;

    UseResult::UsedUp
}

fn toggle_equipment(
    inventory_id: usize,
    _objects: &mut [GameObject],
    game: &mut Game,
    _tcod: &mut Tcod,
) -> UseResult {
    let equipment = match game.inventory[inventory_id].equipment {
        Some(equipment) => equipment,
        None => return UseResult::Cancelled,
    };

    if equipment.equipped {
        game.inventory[inventory_id].dequip(&mut game.log);
    } else {
        if let Some(old_equipment) = get_equipped_in_slot(equipment.slot, game) {
            game.inventory[old_equipment].dequip(&mut game.log);
        }

        game.inventory[inventory_id].equip(&mut game.log);
    }

    UseResult::UsedAndKept
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

/// Returns a vaue that depends on current dungeon level. The table specifies what
/// value occurs at each level, the default is 0
fn from_dungeon_level(table: &[Transition], level: u32) -> u32 {
    table
        .iter()
        .rev()
        .find(|transition| level >= transition.level)
        .map_or(0, |transition| transition.value)
}

/// Advance to the next level
fn next_level(tcod: &mut Tcod, objects: &mut Vec<GameObject>, game: &mut Game) {
    use constants::gui::menus::next_level;

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
    use constants::gui::menus::level_up;

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
            choice = menu(
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
    dagger.item = Some(Item::Sword);
    dagger.equipment = Some(Equipment {
        equipped: true,
        slot: Slot::LeftHand,
        hp_bonus: dagger::HP_BONUS,
        defense_bonus: dagger::DEFENSE_BONUS,
        power_bonus: dagger::POWER_BONUS,
    });
    game.inventory.push(dagger);

    initialize_fov(&game, tcod);

    game.log.add(constants::gui::WELCOME_MESSAGE, colors::RED);

    (game_objects, game)
}

fn initialize_fov(game: &Game, tcod: &mut Tcod) {
    for y in 0..constants::gui::MAP_HEIGHT {
        for x in 0..constants::gui::MAP_WIDTH {
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

        render_all(&mut tcod, &game_objects, &mut game);

        // Clear the GameObjects once their position is moved to the visible screen.
        // If we do this earlier or later we won't erase the last pos.
        for object in &game_objects {
            object.clear(&mut tcod.con);
        }

        // Handle player movement
        let action = handle_keys(key, &mut tcod, &mut game, &mut game_objects);

        if action == PlayerAction::Exit {
            save_game(&game_objects, game).unwrap();
            break;
        }

        if game_objects[GameConstants::PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..game_objects.len() {
                if game_objects[id].ai.is_some() {
                    ai_take_turn(id, &mut game_objects, &mut tcod, &mut game);
                }
            }
        }

        level_up(&mut game_objects, game, tcod);
    }
}

fn main_menu(mut tcod: &mut Tcod) {
    use constants::gui::menus::*;
    let img = tcod::image::Image::from_file(main::IMAGE_PATH)
        .ok()
        .expect("Background image not found");

    while !tcod.root.window_closed() {
        // show the image, at twice the regular console resolution
        tcod::image::blit_2x(&img, (0, 0), (-1, -1), &mut tcod.root, (0, 0));

        tcod.root.set_default_foreground(colors::LIGHT_YELLOW);
        tcod.root.print_ex(
            constants::gui::SCREEN_WIDTH / 2,
            constants::gui::SCREEN_HEIGHT / 2 - 4,
            BackgroundFlag::None,
            TextAlignment::Center,
            constants::GAME_TITLE,
        );
        tcod.root.print_ex(
            constants::gui::SCREEN_WIDTH / 2,
            constants::gui::SCREEN_HEIGHT - 2,
            BackgroundFlag::None,
            TextAlignment::Center,
            main::AUTHOR_LINE,
        );

        // show options and wait for the players choice
        let choices = &[main::NEW_GAME, main::CONTINUE, main::QUIT];
        let choice = menu(
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
            Some(1) => match load_game() {
                Ok((objects, mut game)) => {
                    initialize_fov(&game, tcod);
                    play_game(objects, &mut game, tcod);
                }
                Err(_e) => {
                    msgbox("\nNo saved game to load.\n", 24, &mut tcod);
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

fn msgbox(text: &str, width: i32, mut tcod: &mut Tcod) {
    let options: &[&str] = &[];
    menu(text, options, width, &mut tcod);
}

fn save_game(objects: &[GameObject], game: &Game) -> Result<(), Box<dyn Error>> {
    let save_data = serde_json::to_string(&(objects, game))?;
    let mut file = File::create(constants::SAVE_FILE_NAME)?;
    file.write_all(save_data.as_bytes())?;
    Ok(())
}

fn load_game() -> Result<(Vec<GameObject>, Game), Box<dyn Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open(constants::SAVE_FILE_NAME)?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(Vec<GameObject>, Game)>(&json_save_state)?;
    Ok(result)
}

fn main() {
    let root = Root::initializer()
        .font(constants::FONT_PATH, FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(constants::gui::SCREEN_WIDTH, constants::gui::SCREEN_HEIGHT)
        .title(constants::gui::menus::main::GAME_CONSOLE_HEADER)
        .init();

    tcod::system::set_fps(GameConstants::LIMIT_FPS);

    let mut tcod = Tcod {
        root,
        con: Offscreen::new(constants::gui::MAP_WIDTH, constants::gui::MAP_HEIGHT),
        panel: Offscreen::new(constants::gui::SCREEN_WIDTH, constants::gui::PANEL_HEIGHT),
        fov: FovMap::new(constants::gui::MAP_WIDTH, constants::gui::MAP_HEIGHT),
        mouse: Default::default(),
    };

    main_menu(&mut tcod);
}
