//! City teleport services offered by the Conductress NPCs, the Market
//! controller and the newbie guide.
//!
//! Every destination is resolved on the server from the NPC id and the dialog
//! option; the client never supplies a map or a coordinate. The gate
//! coordinates match the landing cells of the portals in
//! `migrations/9_generated_portals.sql`, so a teleport leaves the player on the
//! same spot as walking through the matching gate.

/// The classic fee charged by a Conductress for each trip.
pub const CONDUCTRESS_FEE: u32 = 100;

pub const TWIN_CITY_MAP: u32 = 1002;
pub const MARKET_MAP: u32 = 1036;

/// Where a player arrives in the Market.
pub const MARKET_ENTRY: (u16, u16) = (211, 196);
/// Where the Market controller sends a player that has no recorded return
/// point (for example after a reconnect inside the Market).
pub const TWIN_CITY_CENTER: (u16, u16) = (430, 380);

/// Option shown by the Market controller to leave the Market.
pub const LEAVE_MARKET_OPTION: u8 = 1;

const MARKET_CONTROLLER_ID: u32 = 45;
const KNOW_IT_ALL_ID: u32 = 10010;
const KNOW_IT_ALL_TWIN_CITY_OPTION: u8 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Destination {
    pub name: &'static str,
    pub map_id: u32,
    pub position: (u16, u16),
}

const fn dest(name: &'static str, map_id: u32, x: u16, y: u16) -> Destination {
    Destination {
        name,
        map_id,
        position: (x, y),
    }
}

const MARKET: Destination = dest("Market", MARKET_MAP, MARKET_ENTRY.0, MARKET_ENTRY.1);

/// A Conductress: one per city, each with the gates of her own city plus the
/// Market. Option ids are the 1-based position in `destinations`.
#[derive(Debug)]
pub struct Conductress {
    pub npc_id: u32,
    pub city: &'static str,
    pub destinations: &'static [Destination],
}

pub const CONDUCTRESSES: &[Conductress] = &[
    Conductress {
        npc_id: 10050,
        city: "Twin City",
        destinations: &[
            dest("Phoenix Castle", TWIN_CITY_MAP, 958, 555),
            dest("Desert City", TWIN_CITY_MAP, 69, 473),
            dest("Ape Mountain", TWIN_CITY_MAP, 555, 957),
            dest("Bird Island", TWIN_CITY_MAP, 232, 190),
            dest("Mine Cave", TWIN_CITY_MAP, 53, 399),
            MARKET,
        ],
    },
    Conductress {
        npc_id: 10051,
        city: "Desert City",
        destinations: &[
            dest("Twin City", 1000, 971, 666),
            dest("Mystic Castle", 1000, 85, 323),
            MARKET,
        ],
    },
    Conductress {
        npc_id: 10052,
        city: "Phoenix Castle",
        destinations: &[dest("Twin City", 1011, 11, 376), MARKET],
    },
    Conductress {
        npc_id: 10053,
        city: "Ape Mountain",
        destinations: &[dest("Twin City", 1020, 381, 21), MARKET],
    },
    Conductress {
        npc_id: 10056,
        city: "Bird Island",
        destinations: &[dest("Twin City", 1015, 1010, 710), MARKET],
    },
];

pub fn conductress(npc_id: u32) -> Option<&'static Conductress> {
    CONDUCTRESSES.iter().find(|c| c.npc_id == npc_id)
}

pub fn is_market_controller(npc_id: u32) -> bool {
    npc_id == MARKET_CONTROLLER_ID
}

/// A teleport resolved from an NPC dialog answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Teleport {
    /// A paid trip; `records_market_return` asks the caller to remember the
    /// player's position so the Market controller can send them back.
    Paid {
        destination: Destination,
        fee: u32,
        records_market_return: bool,
    },
    /// The Market controller sends the player back to where they came from.
    LeaveMarket,
    /// A free trip offered by the newbie guide.
    Free(Destination),
}

/// Resolves the teleport for `option_id` chosen in the dialog of `npc_id`.
/// Returns `None` for NPCs and options that do not teleport, which includes
/// the prison Wardens: leaving the prison is not a city service.
pub fn resolve(npc_id: u32, option_id: u8) -> Option<Teleport> {
    if let Some(conductress) = conductress(npc_id) {
        let index = usize::from(option_id).checked_sub(1)?;
        let destination = *conductress.destinations.get(index)?;
        return Some(Teleport::Paid {
            destination,
            fee: CONDUCTRESS_FEE,
            records_market_return: destination.map_id == MARKET_MAP,
        });
    }
    if is_market_controller(npc_id) && option_id == LEAVE_MARKET_OPTION {
        return Some(Teleport::LeaveMarket);
    }
    if npc_id == KNOW_IT_ALL_ID && option_id == KNOW_IT_ALL_TWIN_CITY_OPTION {
        return Some(Teleport::Free(dest(
            "Twin City",
            TWIN_CITY_MAP,
            TWIN_CITY_CENTER.0,
            TWIN_CITY_CENTER.1,
        )));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twin_city_conductress_sends_players_to_the_city_gates() {
        let Some(Teleport::Paid {
            destination,
            fee,
            records_market_return,
        }) = resolve(10050, 2)
        else {
            panic!("option 2 must be a paid teleport");
        };
        assert_eq!(destination, dest("Desert City", 1002, 69, 473));
        assert_eq!(fee, CONDUCTRESS_FEE);
        assert!(!records_market_return);
    }

    #[test]
    fn market_trips_use_the_market_map_and_record_the_return_point() {
        for conductress in CONDUCTRESSES {
            let option = conductress.destinations.len() as u8;
            let Some(Teleport::Paid {
                destination,
                records_market_return,
                ..
            }) = resolve(conductress.npc_id, option)
            else {
                panic!("{} must offer the Market last", conductress.city);
            };
            assert_eq!(destination.map_id, MARKET_MAP);
            assert!(records_market_return);
        }
    }

    #[test]
    fn every_city_conductress_offers_a_way_back_to_twin_city() {
        for conductress in CONDUCTRESSES.iter().filter(|c| c.city != "Twin City") {
            assert_eq!(conductress.destinations[0].name, "Twin City");
            assert_ne!(conductress.destinations[0].map_id, MARKET_MAP);
        }
    }

    #[test]
    fn unknown_options_and_npcs_do_not_teleport() {
        assert_eq!(resolve(10050, 0), None);
        assert_eq!(resolve(10050, 7), None);
        assert_eq!(resolve(10050, 255), None);
        assert_eq!(resolve(45, 2), None);
        // Blacksmith and the prison Wardens.
        assert_eq!(resolve(5, 1), None);
        assert_eq!(resolve(42, 1), None);
        assert_eq!(resolve(140, 1), None);
    }

    #[test]
    fn market_controller_and_guide_keep_their_trips() {
        assert_eq!(resolve(45, LEAVE_MARKET_OPTION), Some(Teleport::LeaveMarket));
        assert_eq!(
            resolve(10010, 3),
            Some(Teleport::Free(dest("Twin City", 1002, 430, 380)))
        );
    }
}
