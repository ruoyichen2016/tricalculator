//! Shared turn economy for deterministic replays; spell effect resolution is separate.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCost {
    Standard,
    Move,
    Swift,
    FullRound,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnBudget {
    standard: u32,
    movement: u32,
    swift: u32,
    full: u32,
}
impl Default for TurnBudget {
    fn default() -> Self {
        Self {
            standard: 1,
            movement: 1,
            swift: 1,
            full: 0,
        }
    }
}
impl TurnBudget {
    pub fn available(&self, action: ActionCost) -> bool {
        match action {
            ActionCost::Standard => self.standard > 0,
            ActionCost::Move => self.movement > 0 || self.standard > 0,
            ActionCost::Swift => self.swift > 0,
            ActionCost::FullRound => self.full > 0 || (self.standard > 0 && self.movement > 0),
        }
    }
    pub fn spend(&mut self, action: ActionCost) -> Result<(), String> {
        if !self.available(action) {
            return Err(format!("No {action:?} action available"));
        }
        match action {
            ActionCost::Standard => self.standard -= 1,
            ActionCost::Move => {
                if self.movement > 0 {
                    self.movement -= 1
                } else {
                    self.standard -= 1
                }
            }
            ActionCost::Swift => self.swift -= 1,
            ActionCost::FullRound => {
                if self.full > 0 {
                    self.full -= 1
                } else {
                    self.standard -= 1;
                    self.movement -= 1
                }
            }
        }
        Ok(())
    }
    pub fn grant(&mut self, action: ActionCost) -> Result<(), String> {
        let value = match action {
            ActionCost::Standard => &mut self.standard,
            ActionCost::Move => &mut self.movement,
            ActionCost::Swift => &mut self.swift,
            ActionCost::FullRound => &mut self.full,
        };
        *value = value.checked_add(1).ok_or("Action budget overflow")?;
        Ok(())
    }
}
