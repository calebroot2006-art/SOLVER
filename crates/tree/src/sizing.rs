use crate::{Chips, MAX_CHIPS, TreeError, reserve};

const MAX_MENU_BYTES: usize = 4096;
const MAX_MENU_ENTRIES: usize = 64;

/// One configured wager size, before minimums, thresholds, and stack limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BetSize {
    /// Fraction of the current pot, or the pot after calling for a raise.
    Pot(f64),
    /// Raise-to multiplier of the highest contribution; valid only for raises.
    PreviousBet(f64),
    /// Initial bet amount, or increment above the highest contribution.
    Additive(Chips),
    /// Commit the entire effective stack.
    AllIn,
}

/// Checked menus for an initial bet and for raises, respectively.
#[derive(Clone, Debug, PartialEq)]
pub struct BetSizeOptions {
    bets: Vec<BetSize>,
    raises: Vec<BetSize>,
}

impl BetSizeOptions {
    /// Validates at most 64 entries per menu, then removes identical options.
    /// Bet fractions must be positive, raise multipliers greater than one, and
    /// additive amounts in `1..=1_000_000_000`. All floating sizes must be finite.
    pub fn new(mut bets: Vec<BetSize>, mut raises: Vec<BetSize>) -> Result<Self, TreeError> {
        validate(&bets, false)?;
        validate(&raises, true)?;
        deduplicate(&mut bets);
        deduplicate(&mut raises);
        Ok(Self { bets, raises })
    }

    /// Initial bet options, in first-occurrence input order.
    #[must_use]
    pub fn bets(&self) -> &[BetSize] {
        &self.bets
    }

    /// Raise options, in first-occurrence input order.
    #[must_use]
    pub fn raises(&self) -> &[BetSize] {
        &self.raises
    }

    pub(crate) fn storage_bytes(&self) -> usize {
        (self.bets.capacity() + self.raises.capacity()) * std::mem::size_of::<BetSize>()
    }
}

impl TryFrom<(&str, &str)> for BetSizeOptions {
    type Error = TreeError;

    fn try_from((bets, raises): (&str, &str)) -> Result<Self, Self::Error> {
        Self::new(parse(bets)?, parse(raises)?)
    }
}

fn validate(menu: &[BetSize], raising: bool) -> Result<(), TreeError> {
    if menu.len() > MAX_MENU_ENTRIES {
        return Err(TreeError::new("size menu exceeds 64 entries"));
    }
    for size in menu {
        let valid = match *size {
            BetSize::Pot(value) => value.is_finite() && value > 0.0,
            BetSize::PreviousBet(value) => raising && value.is_finite() && value > 1.0,
            BetSize::Additive(value) => (1..=MAX_CHIPS).contains(&value),
            BetSize::AllIn => true,
        };
        if !valid {
            return Err(TreeError::new(format!(
                "invalid size {size:?} in {} menu",
                if raising { "raise" } else { "bet" }
            )));
        }
    }
    Ok(())
}

fn deduplicate(menu: &mut Vec<BetSize>) {
    for index in (0..menu.len()).rev() {
        if menu[..index].contains(&menu[index]) {
            menu.remove(index);
        }
    }
}

fn ascii_space(value: char) -> bool {
    value.is_ascii_whitespace() || value == '\u{b}'
}

fn parse(text: &str) -> Result<Vec<BetSize>, TreeError> {
    if text.len() > MAX_MENU_BYTES {
        return Err(TreeError::new("size menu exceeds 4096 bytes"));
    }
    if !text.is_ascii() {
        return Err(TreeError::new("size menu syntax must be ASCII"));
    }
    if text.trim_matches(ascii_space).is_empty() {
        return Ok(Vec::new());
    }
    let count = text.split(',').count();
    if count > MAX_MENU_ENTRIES {
        return Err(TreeError::new("size menu exceeds 64 entries"));
    }
    let mut result = Vec::new();
    reserve(&mut result, count)?;
    for item in text.split(',') {
        let token = item.trim_matches(ascii_space);
        let invalid = || TreeError::new(format!("invalid size token {token:?}"));
        if token.is_empty() || token.chars().any(ascii_space) {
            return Err(invalid());
        }
        let size = match token {
            "a" | "e" => BetSize::AllIn,
            _ => {
                let (number, suffix) = token.split_at(token.len() - 1);
                match suffix {
                    "%" => BetSize::Pot(number.parse::<f64>().map_err(|_| invalid())? / 100.0),
                    "x" => BetSize::PreviousBet(number.parse().map_err(|_| invalid())?),
                    "c" => BetSize::Additive(number.parse().map_err(|_| invalid())?),
                    _ => return Err(invalid()),
                }
            }
        };
        result.push(size);
    }
    Ok(result)
}
