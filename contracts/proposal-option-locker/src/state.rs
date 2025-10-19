use cw_storage_plus::Item;
use proposal_locker_types::{Config, State};

/// Contract configuration
pub const CONFIG: Item<Config> = Item::new("config");

/// Global state (total staked, has_voted flag)
pub const STATE: Item<State> = Item::new("state");
