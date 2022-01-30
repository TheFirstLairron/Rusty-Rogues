use tcod::colors::{self, Color};
use tcod::console::*;

use super::constants;

pub type Map = Vec<Vec<Tile>>;
pub type Messages = Vec<(String, Color)>;

#[derive(Debug, Serialize, Deserialize)]
pub struct GameObject {
    pub x: i32,
    pub y: i32,
    char: char,
    color: Color,
    pub name: String,
    pub blocks: bool,
    pub alive: bool,
    pub fighter: Option<Fighter>,
    pub ai: Option<Ai>,
    pub item: Option<Item>,
    pub always_visible: bool,
    pub level: i32,
    pub equipment: Option<Equipment>,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub blocked: bool,
    pub block_sight: bool,
    pub explored: bool,
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
pub struct Fighter {
    pub base_max_hp: i32,
    pub hp: i32,
    pub base_defense: i32,
    pub base_power: i32,
    pub on_death: DeathCallback,
    pub xp: i32,
}

#[derive(Serialize, Deserialize)]
pub struct Game {
    pub map: Map,
    pub log: Messages,
    pub inventory: Vec<GameObject>,
    pub dungeon_level: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Ai {
    Basic,
    Confused {
        previous_ai: Box<Ai>,
        num_turns: i32,
    },
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

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Equipment {
    pub slot: Slot,
    pub equipped: bool,
    pub power_bonus: i32,
    pub defense_bonus: i32,
    pub hp_bonus: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Slot {
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

pub trait MessageLog {
    fn add<T: Into<String>>(&mut self, message: T, color: Color);
}

impl MessageLog for Vec<(String, Color)> {
    fn add<T: Into<String>>(&mut self, message: T, color: Color) {
        self.push((message.into(), color));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DeathCallback {
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
