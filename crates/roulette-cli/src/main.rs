//! `roulette` — terminal front-end for the headless `roulette_core` engine
//! (TASK-042 rebuild). Every mutation goes through `Engine::apply(&Command)`;
//! the CLI only renders read-only state and relays input, so it is a thin
//! proof that the facade is front-end-ready.

use std::io::BufRead;
use std::sync::Arc;

use roulette_content::{CardDef, Content, SlotColor};

use roulette_core::api::{Command, CustomizeOp, Engine};
use roulette_core::bets::BetType;
use roulette_core::run::map::NodeType;
use roulette_core::run::state::{Difficulty, GameState};

const DEFAULT_SEED: &str = "damned";
const CONTENT_DIR: &str = "content";

fn main() {
    let mut args = std::env::args().skip(1);
    let seed = args.next().unwrap_or_else(|| DEFAULT_SEED.to_string());

    let content = match Content::load_dir(CONTENT_DIR) {
        Ok(c) => c,
        Err(_) => Content::embedded().expect("embedded content validates"),
    };
    let engine = Engine::new(Arc::new(content.clone()), &seed);
    println!("Roulette.OS — Roulette of the Damned (terminal)");
    println!("seed: {seed} | type `help` for commands\n");

    let stdin = std::io::stdin();
    let mut cli = Cli { engine, content };
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if !cli.execute(&line) {
            break;
        }
    }
}

struct Cli {
    engine: Engine,
    content: Content,
}

impl Cli {
    /// Runs one input line. Returns false when the session should end.
    fn execute(&mut self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() {
            return true;
        }
        let mut words = line.split_whitespace();
        let verb = words.next().unwrap_or("");
        let rest: Vec<&str> = words.collect();

        match self.dispatch(verb, &rest) {
            Ok(true) => {}
            Ok(false) => return false,
            Err(e) => println!("✗ {e}"),
        }
        self.render();
        true
    }

    /// Ok(false) = quit; Err = user-facing error.
    fn dispatch(&mut self, verb: &str, rest: &[&str]) -> Result<bool, String> {
        match verb {
            "quit" | "exit" | "q" => Ok(false),
            "help" => {
                self.print_help();
                Ok(true)
            }
            "state" | "map" | "hand" | "bets" | "shop" | "forge" | "event" => {
                // Rendered wholesale by `render()`; a bare verb just refreshes.
                Ok(true)
            }
            "events" => {
                self.dump_events(rest);
                Ok(true)
            }
            _ => {
                let cmd = self.parse_command(verb, rest)?;
                let events = self.engine.apply(&cmd).map_err(|e| e.to_string())?;
                for ev in &events {
                    println!("· {ev:?}");
                }
                Ok(true)
            }
        }
    }

    /// `events [n]` — last n events as JSON (PAT-004: the stream is
    /// serde-able and float-free by construction).
    fn dump_events(&self, rest: &[&str]) {
        let n = rest.first().and_then(|s| s.parse::<usize>().ok()).unwrap_or(10);
        let all = self.engine.events();
        for ev in all.iter().rev().take(n).rev() {
            match serde_json::to_string(ev) {
                Ok(json) => println!("{json}"),
                Err(e) => println!("· {ev:?} (serde: {e})"),
            }
        }
        println!("({} events total)", all.len());
    }

    fn render(&self) {
        match self.engine.run() {
            None => println!("— no run; `start [short|medium|long]` —"),
            Some(run) => {
                println!("─────────────────────────────────────────────");
                println!(
                    "HP {}/{} | ⚡{} | PTS {} | floor {}/{} | {}",
                    run.hp,
                    run.max_hp,
                    run.chips,
                    run.store_points,
                    run.current_floor + 1,
                    run.difficulty.floors(),
                    label_state(run.state),
                );
                match run.state {
                    GameState::LoadoutStore => self.render_loadout(),
                    GameState::Map => self.render_map(),
                    GameState::Combat => self.render_battle(),
                    GameState::Shop => self.render_shop(),
                    GameState::Event => self.render_event(),
                    GameState::Forge => self.render_forge(),
                    GameState::Victory => println!("👑 RUN VICTORY — the wheel is yours."),
                    GameState::GameOver => println!("☠ GAME OVER — the house always wins."),
                    GameState::Menu => {}
                }
                println!("─────────────────────────────────────────────");
            }
        }
    }

    fn render_loadout(&self) {
        let Some(run) = self.engine.run() else { return };
        let Some(offer) = run.loadout_offer.as_ref() else {
            println!("(no loadout offer)");
            return;
        };
        println!("loadout: {} PTS budget, cards {} PTS each", run.store_points, offer.card_price);
        for (i, id) in offer.card_ids.iter().enumerate() {
            match self.card_def(id) {
                Some(def) => println!(
                    "  [{}] {} ⚡{} ({:?}) — {}",
                    i, def.name, def.cost, def.rarity, def.description
                ),
                None => println!("  [{}] {id} (missing def)", i),
            }
        }
        println!("  wheel: {} (free, §4.6 common draft)", offer.wheel_id);
        println!("draft <i> | wheel | done");
    }

    fn render_map(&self) {
        let Some(run) = self.engine.run() else { return };
        let pickable = run.pickable_nodes();
        let pickable: std::collections::BTreeSet<String> = pickable.into_iter().collect();
        for (floor, row) in run.map.floors.iter().enumerate() {
            let nodes = row
                .iter()
                .map(|n| {
                    let mark = if n.completed {
                        "✓"
                    } else if pickable.contains(&n.id) {
                        "▸"
                    } else {
                        "·"
                    };
                    format!("{}{}{}", mark, icon(n.node_type), n.id)
                })
                .collect::<Vec<_>>()
                .join("  ");
            println!("  f{:>2}  {}", floor + 1, nodes);
        }
        println!("pick <node_id> (▸ = reachable)");
    }

    fn render_battle(&self) {
        let Some(b) = self.engine.battle() else { return };
        let intent = b
            .enemy_intent
            .as_ref()
            .map(|i| format!("{:?}×{} “{}”", i.action, i.value, i.description))
            .unwrap_or_else(|| "—".to_string());
        println!(
            "round {}/{}{} phase={:?} turn={:?}",
            b.round,
            b.max_rounds,
            if b.is_sudden_death { " SD" } else { "" },
            b.phase,
            b.turn,
        );
        println!(
            "YOU ♥{}/{} pool⚡{} | FOE ♥{} | intent: {}",
            b.player_hp, b.player_max_hp, b.chips_pool, b.enemy_hp, intent
        );
        for (i, card) in b.hand.iter().enumerate() {
            match self.card_def(&card.def_id) {
                Some(def) => println!(
                    "  [{}] {} ⚡{} — {}{}",
                    i,
                    def.name,
                    def.cost,
                    def.description,
                    if card.temp { " (temp)" } else { "" }
                ),
                None => println!("  [{}] {} (missing def)", i, card.def_id),
            }
        }
        if !b.bets.is_empty() {
            let bets = b
                .bets
                .iter()
                .map(|bet| format!("{}×{}", fmt_bet(&bet.bet_type), bet.amount))
                .collect::<Vec<_>>()
                .join(", ");
            println!("bets: {bets}");
        }
        println!("play <i> | draw | bet <spec> <amt> | unbet <spec> <amt> | clear | rebet | sacrifice | predict | spin");
    }

    fn render_shop(&self) {
        let Some(run) = self.engine.run() else { return };
        let Some(offer) = run.shop_offer.as_ref() else {
            println!("(no shop offer)");
            return;
        };
        for (i, item) in offer.items.iter().enumerate() {
            match item {
                roulette_core::run::shop::ShopItem::Card { def, price } => println!(
                    "  [{}] 🃏 {} ⚡{} ({:?}) — {price}⚡",
                    i, def.name, def.cost, def.rarity
                ),
                roulette_core::run::shop::ShopItem::Wheel { def, price } => {
                    println!("  [{}] 🎡 {} ({:?}) — {price}⚡", i, def.name, def.rarity)
                }
                roulette_core::run::shop::ShopItem::BloodInfusion { price } => {
                    println!("  [{}] 🩸 Blood Infusion (+25 HP / +12⚡) — {price}⚡", i)
                }
            }
        }
        println!("buy <i>");
    }

    fn render_forge(&self) {
        let Some(run) = self.engine.run() else { return };
        let Some(offer) = run.forge_offer.as_ref() else {
            println!("(no forge offer)");
            return;
        };
        for (i, op) in offer.ops.iter().enumerate() {
            let price = match offer.op_price(i) {
                Some(0) => "free".to_string(),
                Some(p) => format!("{p}⚡"),
                None => "?".to_string(),
            };
            println!(
                "  [{}] {:?} {} — {} ({} free left)",
                i, op.rarity, op.name, price, offer.free_ops_remaining
            );
        }
        println!("forge <i> | reroll (5⚡) | level <color>");
        println!("⚙ customizer: custom cycle <slot> | custom add <number> | custom drop <slot> | custom number <slot> <n> | custom save | custom cancel");
    }

    fn render_event(&self) {
        let Some(run) = self.engine.run() else { return };
        let title = run.current_event_title.clone().unwrap_or_default();
        if let Some(flavor) = run.event_flavor.as_ref() {
            println!("❓ {title}\n  {flavor}");
        } else {
            println!("❓ {title}");
        }
        if let Some(ev) = self.content.events.iter().find(|e| e.title == title) {
            for choice in &ev.choices {
                println!("  ▸ {} — {}", choice.id, choice.label);
            }
        }
        println!("choose <choice_id>");
    }

    fn card_def(&self, id: &str) -> Option<&CardDef> {
        self.content.cards.iter().find(|c| c.id == id)
    }
}

fn label_state(state: GameState) -> &'static str {
    match state {
        GameState::Menu => "menu",
        GameState::LoadoutStore => "loadout store",
        GameState::Map => "map",
        GameState::Combat => "combat",
        GameState::Shop => "shop",
        GameState::Event => "event",
        GameState::Forge => "forge",
        GameState::Victory => "victory",
        GameState::GameOver => "game over",
    }
}

fn icon(node: NodeType) -> &'static str {
    match node {
        NodeType::Combat => "💀",
        NodeType::Elite => "👹",
        NodeType::Shop => "⚡",
        NodeType::Event => "❓",
        NodeType::Forge => "🔥",
        NodeType::Boss => "👑",
    }
}

fn fmt_bet(bet: &BetType) -> String {
    match bet {
        BetType::Red => "red".into(),
        BetType::Black => "black".into(),
        BetType::Green => "green".into(),
        BetType::Number(n) => format!("#{n}"),
        BetType::Odd => "odd".into(),
        BetType::Even => "even".into(),
        BetType::Dozen(d) => format!("d{d}"),
        BetType::Column(c) => format!("c{c}"),
        BetType::Gold => "gold".into(),
        BetType::Purple => "purple".into(),
        BetType::Cyan => "cyan".into(),
        BetType::Crimson => "crimson".into(),
    }
}

fn parse_bet(spec: &str) -> Result<BetType, String> {
    match spec {
        "red" => Ok(BetType::Red),
        "black" => Ok(BetType::Black),
        "green" => Ok(BetType::Green),
        "odd" => Ok(BetType::Odd),
        "even" => Ok(BetType::Even),
        "gold" => Ok(BetType::Gold),
        "purple" => Ok(BetType::Purple),
        "cyan" => Ok(BetType::Cyan),
        "crimson" => Ok(BetType::Crimson),
        _ => {
            let num = spec
                .trim_start_matches(['n', '#'])
                .parse::<u32>()
                .map_err(|_| {
                    format!("unknown bet `{spec}` (red|black|green|odd|even|gold|purple|cyan|crimson|n<N>|d<1-3>|c<1-3>)")
                })?;
            Ok(BetType::Number(num))
        }
    }
}

fn parse_color(spec: &str) -> Result<SlotColor, String> {
    match spec {
        "red" => Ok(SlotColor::Red),
        "black" => Ok(SlotColor::Black),
        "green" => Ok(SlotColor::Green),
        "gold" => Ok(SlotColor::Gold),
        "purple" => Ok(SlotColor::Purple),
        "cyan" => Ok(SlotColor::Cyan),
        "crimson" => Ok(SlotColor::Crimson),
        _ => Err(format!("unknown color `{spec}` (red|black|green|gold|purple|cyan|crimson)")),
    }
}

fn parse_usize(spec: &str, what: &str) -> Result<usize, String> {
    spec.parse::<usize>().map_err(|_| format!("{what} must be an integer, got `{spec}`"))
}

impl Cli {
    /// Resolves `draft <i>`: a loadout-offer index becomes its card id.
    fn loadout_card_id(&self, spec: &str) -> Option<String> {
        let idx = spec.parse::<usize>().ok()?;
        let offer = self.engine.run()?.loadout_offer.as_ref()?;
        offer.card_ids.get(idx).cloned()
    }
}

impl Cli {
    fn parse_command(&self, verb: &str, rest: &[&str]) -> Result<Command, String> {
        let arg =
            |i: usize| rest.get(i).copied().ok_or_else(|| format!("`{verb}` missing argument"));
        match verb {
        "start" => {
            let difficulty = match rest.first().copied().unwrap_or("short") {
                "short" => Difficulty::Short,
                "medium" => Difficulty::Medium,
                "long" => Difficulty::Long,
                other => return Err(format!("unknown difficulty `{other}` (short|medium|long)")),
            };
            Ok(Command::StartRun { difficulty })
        }
        "draft" => {
            // `draft <i>` resolves the loadout-offer index to a card id.
            let spec = arg(0)?;
            let card_id = match self.loadout_card_id(spec) {
                Some(id) => id,
                None => spec.to_string(),
            };
            Ok(Command::DraftCard { card_id })
        }
        "wheel" => Ok(Command::DraftWheel),
        "done" => Ok(Command::FinishLoadout),
        "pick" => Ok(Command::PickNode { node_id: arg(0)?.to_string() }),
        "play" => Ok(Command::PlayCard { hand_index: parse_usize(arg(0)?, "hand index")? }),
        "draw" => Ok(Command::BuyDraw),
        "bet" => Ok(Command::PlaceBet {
            bet: parse_bet(arg(0)?)?,
            amount: parse_usize(arg(1)?, "amount")? as u16,
        }),
        "unbet" => Ok(Command::RemoveBet {
            bet: parse_bet(arg(0)?)?,
            amount: parse_usize(arg(1)?, "amount")? as u16,
        }),
        "clear" => Ok(Command::ClearBets),
        "rebet" => Ok(Command::Rebet),
        "sacrifice" => Ok(Command::Sacrifice),
        "predict" => Ok(Command::Predict),
        "spin" => Ok(Command::Spin),
        "buy" => Ok(Command::Purchase { item_index: parse_usize(arg(0)?, "item index")? }),
        "forge" => Ok(Command::ForgeTake { op_index: parse_usize(arg(0)?, "op index")? }),
        "reroll" => Ok(Command::ForgeReroll),
        "choose" => Ok(Command::EventChoose { choice_id: arg(0)?.to_string() }),
        "level" => Ok(Command::BuyLevel { color: parse_color(arg(0)?)? }),
        "undo" => Ok(Command::Undo),
        "custom" => {
            let op = match rest.first().copied().unwrap_or("") {
                "cycle" => CustomizeOp::CycleColor { slot: parse_usize(arg(1)?, "slot")? },
                "add" => CustomizeOp::AddSlot { number: parse_usize(arg(1)?, "number")? as u32 },
                "drop" => CustomizeOp::RemoveSlot { slot: parse_usize(arg(1)?, "slot")? },
                "number" => CustomizeOp::SetNumber {
                    slot: parse_usize(arg(1)?, "slot")?,
                    number: parse_usize(arg(2)?, "number")? as u32,
                },
                "save" => CustomizeOp::Save,
                "cancel" => CustomizeOp::Cancel,
                other => {
                    return Err(format!(
                        "unknown customizer op `{other}` (cycle|add|drop|number|save|cancel)"
                    ))
                }
            };
            Ok(Command::Customize { op })
        }
        other => Err(format!(
            "unknown command `{other}` — try `help` (start|pick|play|draw|bet|unbet|clear|rebet|sacrifice|predict|spin|buy|forge|reroll|choose|level|custom|undo|events|state|map|hand|bets|shop|forge|event|quit)"
        )),
    }
    }

    fn print_help(&self) {
        println!(
            "commands:\n\
         start [short|medium|long]   begin a run (loadout → map)\n\
         draft <i> | wheel | done    loadout store: draft card i, wheel, finish\n\
         pick <node_id>              move along the map (▸ nodes are reachable)\n\
         play <i> | draw             play hand card i / buy an extra draw\n\
         bet <spec> <amt>            spec: red|black|green|odd|even|gold|purple|cyan|crimson|n<N>\n\
         unbet <spec> <amt> | clear | rebet | sacrifice\n\
         predict | spin              arm the prediction sector, then resolve the round\n\
         buy <i> | forge <i> | reroll | choose <id> | level <color>\n\
         custom cycle|add|drop|number|save|cancel   §4.8 wheel customizer (forge)\n\
         undo                        revert the last command\n\
         events [n]                  last n events as JSON\n\
         state | map | hand | bets | shop | forge | event | quit"
        );
    }
}
