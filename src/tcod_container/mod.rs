use tcod::console::*;
use tcod::input::Mouse;
use tcod::map::Map as FovMap;

pub struct Tcod {
    pub root: Root,
    pub con: Offscreen,
    pub panel: Offscreen,
    pub fov: FovMap,
    pub mouse: Mouse,
}