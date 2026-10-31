//! Print a live Snapshot of The Bazaar and the current run's real id, to check the game-memory
//! reader by hand against the game's screen.
//!
//! `cargo run -p bazaar-tracker --example snapshot [-- --watch]`. With `--watch` it prints again
//! every two seconds. Check it in a shop, in a combat and in a PvP fight.

use std::time::{Duration, Instant};

use bazaar_tracker::memory::ConnectionChange;
use bazaar_tracker::memory::game::BazaarMemory;
use bazaar_tracker::memory::{GameMemory, Item, Snapshot};

fn main() {
    let watch = std::env::args().any(|arg| arg == "--watch");
    let mut memory = BazaarMemory::new();
    let (sender, changes) = std::sync::mpsc::channel();
    memory.report_changes_to(sender);

    let started = Instant::now();
    while !memory.is_connected() {
        memory.tick();
        if started.elapsed() > Duration::from_secs(10) {
            eprintln!("snapshot: could not attach to The Bazaar; is it running?");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    loop {
        for change in changes.try_iter() {
            match change {
                ConnectionChange::Connected { pid, unity_version } => println!("[attached to pid {pid}, offsets {unity_version}]"),
                ConnectionChange::Disconnected { reason } => println!("[detached: {reason}]"),
            }
        }
        print_once(&mut memory);
        if !watch {
            break;
        }
        println!();
        std::thread::sleep(Duration::from_secs(2));
        memory.tick();
    }
}

fn print_once(memory: &mut BazaarMemory) {
    match memory.read_run_init() {
        Ok(Some(init)) => println!("run id: {:?}  mode: {:?}  build: {:?}  environment: {:?}", init.run_id, init.play_mode, init.build_id, init.environment),
        Ok(None) => println!("run id: not available yet"),
        Err(error) => println!("run id: {error}"),
    }
    match memory.read_selected_hero() {
        Ok(hero) => println!("selected hero: {hero:?}"),
        Err(error) => println!("selected hero: {error}"),
    }
    match memory.read_board() {
        Ok(Some(board)) => {
            println!("run state: {:?}  storage open: {}  on offer: {:?}", board.run_state, board.is_storage_open, board.selection_set);
            println!("player stats: {:?}", board.player.stats);
            print_side("player", &board.player);
            println!("opponent stats: {:?}", board.opponent.stats);
            print_side("opponent", &board.opponent);
        }
        Ok(None) => println!("board: not available yet (no run in progress?)"),
        Err(error) => println!("board: {error}"),
    }
    match memory.read_pvp_opponent() {
        Ok(Some(opponent)) => println!("pvp opponent: {opponent:?}"),
        Ok(None) => println!("pvp opponent: none"),
        Err(error) => println!("pvp opponent: {error}"),
    }
    match memory.read_player_rank() {
        Ok(Some(rank)) => println!("rank: {rank:?}"),
        Ok(None) => println!("rank: not available yet"),
        Err(error) => println!("rank: {error}"),
    }
    match memory.read_combat_result() {
        Ok(Some(result)) => println!("last combat: {result:?}"),
        Ok(None) => println!("last combat: none"),
        Err(error) => println!("last combat: {error}"),
    }
}

fn print_side(label: &str, side: &Snapshot) {
    for (section, items) in [("board", &side.board), ("stash", &side.stash), ("skills", &side.skills)] {
        println!("{label} {section} ({}):", items.len());
        for item in items {
            println!("  {}", describe(item));
        }
    }
}

fn describe(item: &Item) -> String {
    let attributes: Vec<_> = item.attributes.iter().map(|(name, value)| format!("{name}={value}")).collect();
    format!(
        "{} {} {} {} socket {:?} ench {:?} tags {:?} [{}]",
        item.instance_id,
        item.card_type,
        item.tier,
        item.size,
        item.socket,
        item.enchantment,
        item.tags,
        attributes.join(", ")
    )
}
