//! Event resolution (§9.4, TASK-038): one random event per event node; a
//! choice's `EffectKind`s mutate the run.

use roulette_content::schema::{CardDef, EffectKind, EventDef};

/// Outcome log of applying an event choice (display-oriented).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventOutcome {
    pub hp_lost: u16,
    pub chips_gained: u16,
    pub cards_granted: Vec<String>,
    pub notes: Vec<String>,
}

impl EventOutcome {
    pub fn is_noop(&self) -> bool {
        self.hp_lost == 0 && self.chips_gained == 0 && self.cards_granted.is_empty()
    }
}

/// Applies the effects of the chosen event choice to the run.
/// `hp`/`chips` are passed as mutable run fields so this stays decoupled from
/// `RunState` (which wraps it); returns the outcome log.
pub fn apply_choice_effects(
    effects: &[EffectKind],
    cards: &[CardDef],
    hp: &mut u16,
    max_hp: u16,
    chips: &mut u16,
    deck: &mut Vec<crate::battle::state::CardInstance>,
) -> EventOutcome {
    let mut out = EventOutcome::default();
    for effect in effects {
        match effect {
            EffectKind::LossHp { amount } => {
                *hp = hp.saturating_sub(*amount);
                out.hp_lost += amount;
            }
            EffectKind::GrantChips { amount } => {
                *chips = chips.saturating_add(*amount);
                out.chips_gained += amount;
            }
            EffectKind::GrantCard { card_id } => {
                if cards.iter().any(|c| &c.id == card_id) {
                    deck.push(crate::battle::state::CardInstance {
                        def_id: card_id.clone(),
                        marked_slots: Vec::new(),
                        temp: false,
                        retained: false,
                        cost_override: None,
                    });
                    out.cards_granted.push(card_id.clone());
                } else {
                    out.notes.push(format!("unknown card {card_id}"));
                }
            }
            other => {
                out.notes.push(format!("event effect {:?} not applicable at run level", other))
            }
        }
    }
    let _ = max_hp;
    out
}

/// Picks a random event def from the pool (§9.4: framework supports N).
pub fn pick_event<'a>(events: &'a [EventDef], rng: &mut crate::rng::Rng) -> Option<&'a EventDef> {
    rng.pick(events)
}
