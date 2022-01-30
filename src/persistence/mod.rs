use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};

use crate::game_objects::{Game, GameObject};
use crate::constants;

pub fn save_game(objects: &[GameObject], game: &Game) -> Result<(), Box<dyn Error>> {
    let save_data = serde_json::to_string(&(objects, game))?;
    let mut file = File::create(constants::SAVE_FILE_NAME)?;
    file.write_all(save_data.as_bytes())?;
    Ok(())
}

pub fn load_game() -> Result<(Vec<GameObject>, Game), Box<dyn Error>> {
    let mut json_save_state = String::new();
    let mut file = File::open(constants::SAVE_FILE_NAME)?;
    file.read_to_string(&mut json_save_state)?;
    let result = serde_json::from_str::<(Vec<GameObject>, Game)>(&json_save_state)?;
    Ok(result)
}