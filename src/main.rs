extern crate rand;
extern crate tcod;

use tcod::colors::{self, Color};
use tcod::console::*;
use tcod::input::Key;
use tcod::input::KeyCode::*;
use tcod::input::{self, Event, Mouse};
use tcod::map::{FovAlgorithm, Map as FovMap};

use std::cmp;

use rand::Rng;

const SCREEN_WIDTH: i32 = 80;
const SCREEN_HEIGHT: i32 = 50;

const INVENTORY_WIDTH: i32 = 50;

const MAP_WIDTH: i32 = 80;
const MAP_HEIGHT: i32 = 43;

const ROOM_MAX_SIZE: i32 = 10;
const ROOM_MIN_SIZE: i32 = 6;
const MAX_ROOMS: i32 = 30;

const FOV_ALGO: FovAlgorithm = FovAlgorithm::Basic;
const FOV_LIGHT_WALLS: bool = true;
const TORCH_RADIUS: i32 = 10;

const PLAYER: usize = 0;
const HEAL_AMOUNT: i32 = 4;

const MAX_ROOM_MONSTERS: i32 = 3;
const MAX_ROOM_ITEMS: i32 = 2;

const LIMIT_FPS: i32 = 20;

// sizes and coordinates relevant for the GUI
const BAR_WIDTH: i32 = 20;
const PANEL_HEIGHT: i32 = 7;
const PANEL_Y: i32 = SCREEN_HEIGHT - PANEL_HEIGHT;

// Message Log Constants
const MSG_X: i32 = BAR_WIDTH + 2;
const MSG_WIDTH: i32 = SCREEN_WIDTH - BAR_WIDTH - 2;
const MSG_HEIGHT: usize = PANEL_HEIGHT as usize - 1;

const COLOR_DARK_WALL: Color = Color { r: 0, g: 0, b: 100 };
const COLOR_LIGHT_WALL: Color = Color {
    r: 130,
    g: 110,
    b: 50,
};
const COLOR_DARK_GROUND: Color = Color {
    r: 50,
    g: 50,
    b: 150,
};
const COLOR_LIGHT_GROUND: Color = Color {
    r: 200,
    g: 180,
    b: 50,
};

type Map = Vec<Vec<Tile>>;
type Messages = Vec<(String, Color)>;

#[derive(Debug)]
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
        }
    }

    pub fn draw(&self, con: &mut Console) {
        con.set_default_foreground(self.color);
        con.put_char(self.x, self.y, self.char, BackgroundFlag::None);
    }

    pub fn clear(&self, con: &mut Console) {
        con.put_char(self.x, self.y, ' ', BackgroundFlag::None);
    }

    pub fn pos(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn distance_to(&self, other: &GameObject) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        ((dx.pow(2) + dy.pow(2)) as f32).sqrt()
    }

    pub fn take_damage(&mut self, damage: i32, messages: &mut Messages) {
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
                fighter.on_death.callback(self, messages);
            }
        }
    }

    pub fn attack(&mut self, target: &mut GameObject, messages: &mut Messages) {
        // A simple formula for attack damage
        let damage = self.fighter.map_or(0, |f| f.power) - target.fighter.map_or(0, |f| f.defense);

        if damage > 0 {
            // Make the target take some damage
            message(
                messages,
                format!(
                    "{} attacks {} for {} hit points",
                    self.name, target.name, damage
                ),
                colors::WHITE,
            );
            target.take_damage(damage, messages);
        } else {
            message(
                messages,
                format!(
                    "{} attacks {} but it has no effect!",
                    self.name, target.name
                ),
                colors::WHITE,
            );
        }
    }

    pub fn heal(&mut self, amount: i32) {
        if let Some(ref mut fighter) = self.fighter {
            fighter.hp += amount;

            if fighter.hp > fighter.max_hp {
                fighter.hp = fighter.max_hp;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fighter {
    max_hp: i32,
    hp: i32,
    defense: i32,
    power: i32,
    on_death: DeathCallback,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum DeathCallback {
    Player,
    Monster,
}

impl DeathCallback {
    fn callback(self, object: &mut GameObject, messages: &mut Messages) {
        use DeathCallback::*;
        let callback: fn(&mut GameObject, &mut Messages) = match self {
            Player => player_death,
            Monster => monster_death,
        };

        callback(object, messages);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ai;

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Item {
    Heal,
}

enum UseResult {
    UsedUp,
    Cancelled,
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

fn main() {
    let mut root = Root::initializer()
        .font("arial10x10.png", FontLayout::Tcod)
        .font_type(FontType::Greyscale)
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Rust/libtcod tutorial")
        .init();
    tcod::system::set_fps(LIMIT_FPS);
    let mut con = Offscreen::new(SCREEN_WIDTH, SCREEN_HEIGHT);
    con.set_default_foreground(colors::WHITE);
    let mut panel = Offscreen::new(SCREEN_WIDTH, PANEL_HEIGHT);

    let mut player = GameObject::new(0, 0, '@', "Player", colors::WHITE, true);
    player.alive = true;
    player.fighter = Some(Fighter {
        max_hp: 30,
        hp: 30,
        defense: 2,
        power: 5,
        on_death: DeathCallback::Player,
    });

    let mut game_objects = vec![player];

    let mut map = create_map(&mut game_objects);

    let mut fov_map = FovMap::new(MAP_WIDTH, MAP_HEIGHT);
    // let mut previous_player_position = (-1, -1);
    let mut mouse = Default::default();
    let mut key = Default::default();

    let mut inventory: Vec<GameObject> = vec![];

    // create the list of game messages and their colors, starts empty
    let mut messages: Messages = vec![];

    message(
        &mut messages,
        "Welcome stranger! Prepare to perish in the Tombs of the Ancient Kings.",
        colors::RED,
    );

    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            fov_map.set(
                x,
                y,
                !map[x as usize][y as usize].block_sight,
                !map[x as usize][y as usize].blocked,
            );
        }
    }

    while !root.window_closed() {
        match input::check_for_event(input::MOUSE | input::KEY_PRESS) {
            Some((_, Event::Mouse(m))) => mouse = m,
            Some((_, Event::Key(k))) => key = k,
            _ => key = Default::default(),
        }

        render_all(
            &mut root,
            &mut con,
            &mut panel,
            mouse,
            &mut game_objects,
            &mut map,
            &messages,
            &mut fov_map,
        );

        // Clear the GameObjects once their position is moved to the visible screen.
        // If we do this earlier or later we won't erase the last pos.
        for object in &game_objects {
            object.clear(&mut con);
        }

        // Handle player movement
        let action = handle_keys(
            key,
            &mut root,
            &map,
            &mut game_objects,
            &mut inventory,
            &mut messages,
        );

        if action == PlayerAction::Exit {
            break;
        }

        if game_objects[PLAYER].alive && action != PlayerAction::DidntTakeTurn {
            for id in 0..game_objects.len() {
                if game_objects[id].ai.is_some() {
                    ai_take_turn(id, &map, &mut game_objects, &fov_map, &mut messages);
                }
            }
        }
    }
}

fn handle_keys(
    key: Key,
    root: &mut Root,
    map: &Map,
    objects: &mut Vec<GameObject>,
    inventory: &mut Vec<GameObject>,
    messages: &mut Messages,
) -> PlayerAction {
    use PlayerAction::*;

    let player_alive = objects[PLAYER].alive;

    match (key, player_alive) {
        (Key { code: Up, .. }, true) => {
            player_move_or_attack(0, -1, map, objects, messages);
            TookTurn
        }
        (Key { code: Down, .. }, true) => {
            player_move_or_attack(0, 1, map, objects, messages);
            TookTurn
        }
        (Key { code: Left, .. }, true) => {
            player_move_or_attack(-1, 0, map, objects, messages);
            TookTurn
        }
        (Key { code: Right, .. }, true) => {
            player_move_or_attack(1, 0, map, objects, messages);
            TookTurn
        }
        (Key { printable: 'g', .. }, true) => {
            // pick up an item
            let item_id = objects
                .iter()
                .position(|object| object.pos() == objects[PLAYER].pos() && object.item.is_some());

            if let Some(item_id) = item_id {
                pick_item_up(item_id, objects, inventory, messages);
            }

            DidntTakeTurn
        }
        (Key { printable: 'i', .. }, true) => {
            // show the inventory: if an item is selected, use it
            let inventory_index = inventory_menu(
                inventory,
                "Press the key next to an item to use it, or any other to cancel. \n",
                root,
            );

            if let Some(inventory_index) = inventory_index {
                use_item(inventory_index, inventory, objects, messages)
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
            let fullscreen = root.is_fullscreen();
            root.set_fullscreen(!fullscreen);
            DidntTakeTurn
        }
        (Key { code: Escape, .. }, _) => Exit,
        _ => DidntTakeTurn,
    }
}

fn render_all(
    root: &mut Root,
    con: &mut Offscreen,
    panel: &mut Offscreen,
    mouse: Mouse,
    game_objects: &[GameObject],
    map: &mut Map,
    messages: &Messages,
    fov_map: &mut FovMap
) {
    // originally checked if user moved, but that caused a bug: every action was delayed by one turn. No observable adverse effects from removing the check.
    let player = &game_objects[PLAYER];
    fov_map.compute_fov(player.x, player.y, TORCH_RADIUS, FOV_LIGHT_WALLS, FOV_ALGO);

    // Go through all tiles and set their background color
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            // check if it's a wall by checking if it blocks sight
            let visible = fov_map.is_in_fov(x, y);
            let is_wall = map[x as usize][y as usize].block_sight;
            let color = match (visible, is_wall) {
                // Outside FOV
                (false, true) => COLOR_DARK_WALL,
                (false, false) => COLOR_DARK_GROUND,
                // Inside FOV
                (true, true) => COLOR_LIGHT_WALL,
                (true, false) => COLOR_LIGHT_GROUND,
            };

            let is_explored = &mut map[x as usize][y as usize].explored;

            if visible {
                // It's visible, set it to explored
                *is_explored = true;
            }

            if *is_explored {
                con.set_char_background(x, y, color, BackgroundFlag::Set);
            }
        }
    }

    // Draw the GameObjects
    let mut to_draw: Vec<_> = game_objects
        .iter()
        .filter(|item| fov_map.is_in_fov(item.x, item.y))
        .collect();
    // Sort so that non-blocking objets come first
    to_draw.sort_by(|item1, item2| item1.blocks.cmp(&item2.blocks));
    // Draw the items in the list
    for object in to_draw {
        // only render if in FOV
        if fov_map.is_in_fov(object.x, object.y) {
            object.draw(con);
        }
    }

    // Blit onto the actual screen
    blit(
        con,
        (0, 0),
        (SCREEN_WIDTH, SCREEN_HEIGHT),
        root,
        (0, 0),
        1.0,
        1.0,
    );

    panel.set_default_background(colors::BLACK);
    panel.clear();

    // Print the game messages, one line at a time
    let mut y = MSG_HEIGHT as i32;

    for &(ref msg, color) in messages.iter().rev() {
        let msg_height = panel.get_height_rect(MSG_X, y, MSG_WIDTH, 0, msg);
        y -= msg_height;

        if y < 0 {
            break;
        }

        panel.set_default_foreground(color);
        panel.print_rect(MSG_X, y, MSG_WIDTH, 0, msg);
    }

    // Show the players stats
    let hp = game_objects[PLAYER].fighter.map_or(0, |f| f.hp);
    let max_hp = game_objects[PLAYER].fighter.map_or(0, |f| f.max_hp);
    render_bar(
        panel,
        1,
        1,
        BAR_WIDTH,
        "HP",
        hp,
        max_hp,
        colors::LIGHT_RED,
        colors::DARKER_RED,
    );

    // Display the names of the objects under th mouse
    panel.set_default_foreground(colors::LIGHT_GREY);
    panel.print_ex(
        1,
        0,
        BackgroundFlag::None,
        TextAlignment::Left,
        get_names_under_mouse(mouse, game_objects, fov_map),
    );

    blit(
        panel,
        (0, 0),
        (SCREEN_WIDTH, PANEL_HEIGHT),
        root,
        (0, PANEL_Y),
        1.0,
        1.0,
    );

    // Make the console actually visible
    root.flush();
}

fn create_map(objects: &mut Vec<GameObject>) -> Map {
    let mut map = vec![vec![Tile::wall(); MAP_HEIGHT as usize]; MAP_WIDTH as usize];
    let mut rooms = vec![];

    for _ in 0..MAX_ROOMS {
        // Random width and height
        let w = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);
        let h = rand::thread_rng().gen_range(ROOM_MIN_SIZE, ROOM_MAX_SIZE + 1);

        let x = rand::thread_rng().gen_range(0, MAP_WIDTH - w);
        let y = rand::thread_rng().gen_range(0, MAP_HEIGHT - h);

        let new_room = Rect::new(x, y, w, h);
        let failed = rooms
            .iter()
            .any(|other_room| new_room.intersects_with(other_room));

        if !failed {
            // There are no intersections so we can process this
            create_room(new_room, &mut map);
            place_objects(new_room, &mut map, objects);

            let (center_x, center_y) = new_room.center();

            if rooms.is_empty() {
                objects[PLAYER].set_pos(center_x, center_y)
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

fn place_objects(room: Rect, map: &Map, objects: &mut Vec<GameObject>) {
    let num_monsters = rand::thread_rng().gen_range(0, MAX_ROOM_MONSTERS + 1);
    let num_items = rand::thread_rng().gen_range(0, MAX_ROOM_ITEMS + 1);

    for _ in 0..num_monsters {
        // Choose Random spot
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        let mut monster = if rand::random::<f32>() < 0.8 {
            // 80% chance of getting an orc
            let mut orc = GameObject::new(x, y, 'o', "Orc", colors::DESATURATED_GREEN, true);
            orc.fighter = Some(Fighter {
                max_hp: 10,
                hp: 10,
                defense: 0,
                power: 3,
                on_death: DeathCallback::Monster,
            });
            orc.ai = Some(Ai);
            orc
        } else {
            // Otherwise a troll
            let mut troll = GameObject::new(x, y, 'T', "Troll", colors::DARKER_GREEN, true);
            troll.fighter = Some(Fighter {
                max_hp: 16,
                hp: 16,
                defense: 1,
                power: 4,
                on_death: DeathCallback::Monster,
            });
            troll.ai = Some(Ai);
            troll
        };

        monster.alive = true;

        objects.push(monster);
    }

    for _ in 0..num_items {
        // choose random spot for this item
        let x = rand::thread_rng().gen_range(room.x1 + 1, room.x2);
        let y = rand::thread_rng().gen_range(room.y1 + 1, room.y2);

        if !is_blocked(x, y, map, objects) {
            // Create a healing potion
            let mut object = GameObject::new(x, y, '!', "healing potion", colors::VIOLET, false);
            object.item = Some(Item::Heal);
            objects.push(object);
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

fn move_by(id: usize, dx: i32, dy: i32, map: &Map, objects: &mut [GameObject]) {
    let (x, y) = objects[id].pos();

    if !is_blocked(x + dx, y + dy, map, objects) {
        objects[id].set_pos(x + dx, y + dy);
    }
}

fn move_towards(id: usize, target_x: i32, target_y: i32, map: &Map, objects: &mut [GameObject]) {
    // Vector from this object to the target and distance
    let dx = target_x - objects[id].x;
    let dy = target_y - objects[id].y;
    let distance = ((dx.pow(2) + dy.pow(2)) as f32).sqrt();

    // Normalize it to length 1 (preserving direction), then round it and convert to int so the movement is restricted to the grid
    let dx = (dx as f32 / distance).round() as i32;
    let dy = (dy as f32 / distance).round() as i32;
    move_by(id, dx, dy, map, objects);
}

fn player_move_or_attack(
    dx: i32,
    dy: i32,
    map: &Map,
    objects: &mut [GameObject],
    messages: &mut Messages,
) {
    let x = objects[PLAYER].x + dx;
    let y = objects[PLAYER].y + dy;

    let target_id = objects
        .iter()
        .position(|object| object.fighter.is_some() && object.pos() == (x, y));

    match target_id {
        Some(target_id) => {
            let (player, target) = mut_two(PLAYER, target_id, objects);
            player.attack(target, messages);
        }
        None => move_by(PLAYER, dx, dy, map, objects),
    }
}

fn pick_item_up(
    object_id: usize,
    objects: &mut Vec<GameObject>,
    inventory: &mut Vec<GameObject>,
    messages: &mut Messages,
) {
    if inventory.len() >= 26 {
        message(
            messages,
            format!(
                "Your inventory is full, cannot pick up {}",
                objects[object_id].name
            ),
            colors::RED,
        );
    } else {
        let item = objects.swap_remove(object_id);

        message(
            messages,
            format!("You picked up a {}!", item.name),
            colors::GREEN,
        );

        inventory.push(item);
    }
}

fn ai_take_turn(
    monster_id: usize,
    map: &Map,
    objects: &mut [GameObject],
    fov_map: &FovMap,
    messages: &mut Messages,
) {
    // a basic monster takes its turn. If you can see it, it can see you.
    let (mon_x, mon_y) = objects[monster_id].pos();
    if fov_map.is_in_fov(mon_x, mon_y) {
        if objects[monster_id].distance_to(&objects[PLAYER]) >= 2.0 {
            // move towards the player if far away
            let (player_x, player_y) = objects[PLAYER].pos();
            move_towards(monster_id, player_x, player_y, map, objects);
        } else if objects[PLAYER].fighter.map_or(false, |f| f.hp > 0) {
            // Close enough, attack! (if the player is still alive)
            let (monster, player) = mut_two(monster_id, PLAYER, objects);
            monster.attack(player, messages);
        }
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

fn player_death(player: &mut GameObject, messages: &mut Messages) {
    // The game ended!
    message(messages, "You died!", colors::RED);

    player.char = '%';
    player.color = colors::DARK_RED;
    player.name = "Corpse of player".to_string();
}

fn monster_death(monster: &mut GameObject, messages: &mut Messages) {
    // Transform into corpse. Won't block, can't attack/be attacked, and doesn't move
    message(
        messages,
        format!("{} is dead!", monster.name),
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

fn message<T: Into<String>>(messages: &mut Messages, message: T, color: Color) {
    // If the buffer is full, remove the first message to make room for the new one
    if messages.len() == MSG_HEIGHT {
        messages.remove(0);
    }

    // Add the new line as a tuple with the color and the text
    messages.push((message.into(), color));
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

fn menu<T: AsRef<str>>(header: &str, options: &[T], width: i32, root: &mut Root) -> Option<usize> {
    assert!(
        options.len() <= 26,
        "Cannot have a menu with more than 26 options"
    );

    // calculate total height for the header (after auto-wrap) and one line per option
    let header_height = root.get_height_rect(0, 0, width, SCREEN_HEIGHT, header);
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

    let x = SCREEN_WIDTH / 2 - width / 2;
    let y = SCREEN_HEIGHT / 2 - height / 2;
    tcod::console::blit(&mut window, (0, 0), (width, height), root, (x, y), 1.0, 0.7);

    // present the root console to the player and wait for a key press
    root.flush();
    let key = root.wait_for_keypress(true);

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

fn inventory_menu(inventory: &[GameObject], header: &str, root: &mut Root) -> Option<usize> {
    let options = if inventory.len() == 0 {
        vec!["Inventory is empty.".into()]
    } else {
        inventory.iter().map(|item| item.name.clone()).collect()
    };

    let inventory_index = menu(header, &options, INVENTORY_WIDTH, root);

    // if an item was chosen, return it
    if inventory.len() > 0 {
        inventory_index
    } else {
        None
    }
}

fn use_item(
    inventory_id: usize,
    inventory: &mut Vec<GameObject>,
    objects: &mut [GameObject],
    messages: &mut Messages,
) {
    use Item::*;

    // just call the "use_function" if it is defined
    if let Some(item) = inventory[inventory_id].item {
        let on_use = match item {
            Heal => cast_heal,
        };

        match on_use(inventory_id, objects, messages) {
            UseResult::UsedUp => {
                // destroy after use, unless it was cancelled for some reason
                inventory.remove(inventory_id);
            }
            UseResult::Cancelled => {
                message(messages, "Cancelled", colors::WHITE);
            }
        }
    } else {
        message(
            messages,
            format!("The {} cannot be used.", inventory[inventory_id].name),
            colors::WHITE,
        );
    }
}

fn cast_heal(
    _inventory_id: usize,
    objects: &mut [GameObject],
    messages: &mut Messages,
) -> UseResult {
    // heal the player
    if let Some(fighter) = objects[PLAYER].fighter {
        if fighter.hp == fighter.max_hp {
            message(messages, "You are already at full health.", colors::RED);
            return UseResult::Cancelled;
        }

        message(
            messages,
            "Your wounds start to close up!",
            colors::LIGHT_VIOLET,
        );
        objects[PLAYER].heal(HEAL_AMOUNT);
        return UseResult::UsedUp;
    }

    UseResult::Cancelled
}
