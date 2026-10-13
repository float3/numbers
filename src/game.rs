//! Everyone picks a number, and the rule decides who wins.
//!
//! The oldest of these is the one where the number to aim at is a fraction
//! of what everybody else picks: if the winner is whoever comes closest to
//! a third of the average, then the average cannot be above a third of
//! itself, so the only number everybody can hold is nought — and a room of
//! people never gets there on the first round. Playing it twice is the
//! point. The others in here are different shapes of the same idea: a
//! number whose worth depends on what the rest of the room does.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub const MAX_PLAYERS: usize = 200;
pub const MAX_ROUNDS: usize = 200;
pub const MAX_NAME: usize = 32;
/// The widest range a round can be played over. Wide enough for nought to
/// a hundred, which is the usual board.
pub const LIMIT: f64 = 1_000_000.0;

/// What decides the winner.
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "kebab-case")]
pub enum Rule {
    /// Closest to `num`/`den` of the average. The classic, at 1/3 or 2/3.
    OfTheMean { num: u32, den: u32 },
    /// Closest to `num`/`den` of the middle number. Harder to move with
    /// one silly answer than the average is.
    OfTheMedian { num: u32, den: u32 },
    /// The lowest number nobody else picked. Whole numbers, and there may
    /// be no winner at all.
    LowestUnique,
    /// The highest number nobody else picked.
    HighestUnique,
    /// The highest number that is still below the average. Going high wins
    /// until everybody does it.
    UnderTheMean,
    /// The number the most people picked. The other way round from the
    /// unique rules: here you are trying to meet the room, not dodge it.
    /// Nobody wins if no two people agreed.
    MostPopular,
    /// The number furthest from anybody else's. The one person who went
    /// somewhere nobody else thought of.
    Loneliest,
    /// The highest number wins, but only if everybody's numbers add up to
    /// no more than the limit. Ask for a lot and the room gets nothing.
    UnderTheLimit { limit: f64 },
}

impl Rule {
    /// How the rule is put to the room, in the room's words.
    pub fn label(self) -> String {
        match self {
            Rule::OfTheMean { num: 1, den: 1 } => "closest to the average".into(),
            Rule::OfTheMean { num, den } => format!("closest to {num}/{den} of the average"),
            Rule::OfTheMedian { num: 1, den: 1 } => "closest to the middle number".into(),
            Rule::OfTheMedian { num, den } => {
                format!("closest to {num}/{den} of the middle number")
            }
            Rule::LowestUnique => "the lowest number nobody else picked".into(),
            Rule::HighestUnique => "the highest number nobody else picked".into(),
            Rule::UnderTheMean => "the highest number below the average".into(),
            Rule::MostPopular => "the number the most people picked".into(),
            Rule::Loneliest => "the number furthest from everybody else's".into(),
            Rule::UnderTheLimit { limit } => {
                format!("the highest number, if everybody's together come to no more than {limit}")
            }
        }
    }

    /// Whether picks have to be whole numbers. Picking the same number as
    /// somebody else — or deliberately not — only means anything if there
    /// are a countable number of numbers to pick.
    pub fn whole_only(self) -> bool {
        matches!(
            self,
            Rule::LowestUnique | Rule::HighestUnique | Rule::MostPopular
        )
    }

    fn valid(self) -> Result<(), &'static str> {
        match self {
            Rule::OfTheMean { num, den } | Rule::OfTheMedian { num, den } => {
                if den == 0 || num == 0 || num > den || den > 9 {
                    Err("That fraction is not one of the ones on offer.")
                } else {
                    Ok(())
                }
            }
            Rule::UnderTheLimit { limit } => {
                if limit.is_finite() && limit > 0.0 {
                    Ok(())
                } else {
                    Err("The limit has to be a number above nought.")
                }
            }
            _ => Ok(()),
        }
    }
}

fn mean(picks: &[f64]) -> f64 {
    picks.iter().sum::<f64>() / picks.len() as f64
}

fn median(picks: &[f64]) -> f64 {
    let mut sorted = picks.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    }
}

/// How close two picks have to be before they are the same pick. Picks
/// come off a phone keyboard, so this only matters for a target landing
/// exactly between two of them.
const SAME: f64 = 1e-9;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Pick {
    pub value: f64,
    pub at: u64,
}

#[derive(Serialize, Deserialize)]
pub struct Player {
    pub key: String,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Round {
    pub rule: Rule,
    pub low: f64,
    pub high: f64,
    pub open: bool,
    /// Until this, nobody — the host included — is sent anybody's pick.
    /// The host screen is often the one on the wall.
    pub revealed: bool,
    pub asked: u64,
    /// By player key.
    pub picks: HashMap<String, Pick>,
}

#[derive(Serialize, Deserialize)]
pub struct Game {
    pub join: String,
    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
    pub last_seen: u64,
}

impl rooms::Room for Game {
    fn seen(&mut self, now: u64) {
        self.last_seen = now;
    }

    fn expired(&self, now: u64) -> bool {
        self.last_seen + rooms::DAY_MS < now
    }
}

fn tidy(text: &str, max: usize) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

impl Game {
    pub fn new(now: u64) -> Game {
        Game {
            join: rooms::key::new(),
            players: Vec::new(),
            rounds: Vec::new(),
            last_seen: now,
        }
    }

    pub fn round(&self) -> Option<&Round> {
        self.rounds.last()
    }

    fn round_mut(&mut self) -> Result<&mut Round, &'static str> {
        self.rounds.last_mut().ok_or("No round has been started.")
    }

    pub fn player(&self, key: &str) -> Option<&Player> {
        self.players.iter().find(|p| p.key == key)
    }

    /// As in the estimation game: a name already in the room gives back
    /// that player's key, so a phone that lost its link can walk back in.
    pub fn join(&mut self, name: &str, now: u64) -> Result<String, &'static str> {
        let name = tidy(name, MAX_NAME);
        if name.is_empty() {
            return Err("Please give a name, so the room knows who won.");
        }
        if let Some(player) = self.players.iter().find(|p| p.name == name) {
            return Ok(player.key.clone());
        }
        if self.players.len() >= MAX_PLAYERS {
            return Err("This room is full.");
        }
        let key = rooms::key::new();
        self.players.push(Player {
            key: key.clone(),
            name,
        });
        self.last_seen = now;
        Ok(key)
    }

    /// Starts a round. The round before it stops taking picks.
    pub fn start(&mut self, rule: Rule, low: f64, high: f64, now: u64) -> Result<(), &'static str> {
        rule.valid()?;
        if !low.is_finite() || !high.is_finite() {
            return Err("The range needs two numbers.");
        }
        if low >= high {
            return Err("The range has to go upwards.");
        }
        if low < -LIMIT || high > LIMIT {
            return Err("That range is wider than this will play over.");
        }
        if rule.whole_only() && (low.fract() != 0.0 || high.fract() != 0.0) {
            return Err("This rule is played with whole numbers, so the range needs to be whole.");
        }
        if self.rounds.len() >= MAX_ROUNDS {
            return Err("That is as many rounds as one room holds.");
        }
        if let Some(round) = self.rounds.last_mut() {
            round.open = false;
        }
        self.rounds.push(Round {
            rule,
            low,
            high,
            open: true,
            revealed: false,
            asked: now,
            picks: HashMap::new(),
        });
        Ok(())
    }

    pub fn set_open(&mut self, open: bool) -> Result<(), &'static str> {
        let round = self.round_mut()?;
        if round.revealed {
            return Err("This round is over.");
        }
        round.open = open;
        Ok(())
    }

    /// Shows the room everybody's number and who won. The round stops
    /// taking picks at the same moment.
    pub fn reveal(&mut self) -> Result<(), &'static str> {
        let round = self.round_mut()?;
        round.open = false;
        round.revealed = true;
        Ok(())
    }

    pub fn pick(&mut self, player: &str, value: f64, now: u64) -> Result<(), &'static str> {
        if !value.is_finite() {
            return Err("That is not a number.");
        }
        if self.player(player).is_none() {
            return Err("This link does not belong to anyone in the room.");
        }
        let round = self.round_mut()?;
        if !round.open {
            return Err("This round is closed.");
        }
        if value < round.low || value > round.high {
            return Err("That number is outside the range for this round.");
        }
        if round.rule.whole_only() && value.fract() != 0.0 {
            return Err("This round is played with whole numbers.");
        }
        round
            .picks
            .insert(player.to_string(), Pick { value, at: now });
        Ok(())
    }
}

/// What one player picked, with their name on it. Only ever built for a
/// revealed round.
#[derive(Serialize)]
pub struct Chosen {
    pub name: String,
    pub value: f64,
    pub won: bool,
}

/// How a round came out.
#[derive(Serialize)]
pub struct Outcome {
    pub n: usize,
    pub mean: f64,
    pub median: f64,
    /// The number the room was aiming at, for the rules that have one.
    pub target: Option<f64>,
    /// What the rule was, said again next to the numbers.
    pub label: String,
    pub winners: Vec<String>,
    /// Everybody's pick, lowest first.
    pub picks: Vec<Chosen>,
}

impl Round {
    fn named<'a>(&'a self, players: &'a [Player]) -> Vec<(&'a str, f64)> {
        players
            .iter()
            .filter_map(|p| {
                self.picks
                    .get(&p.key)
                    .map(|pick| (p.name.as_str(), pick.value))
            })
            .collect()
    }

    /// Who won, by the round's rule. Empty when nobody did, which the
    /// unique rules allow: if every number is a duplicate, there is no
    /// lowest unique one.
    fn winners<'a>(&self, named: &[(&'a str, f64)]) -> Vec<&'a str> {
        let values: Vec<f64> = named.iter().map(|(_, value)| *value).collect();
        match self.rule {
            Rule::OfTheMean { .. } | Rule::OfTheMedian { .. } => {
                let Some(target) = self.target(&values) else {
                    return Vec::new();
                };
                let closest = values
                    .iter()
                    .map(|value| (value - target).abs())
                    .fold(f64::INFINITY, f64::min);
                named
                    .iter()
                    .filter(|(_, value)| (value - target).abs() <= closest + SAME)
                    .map(|(name, _)| *name)
                    .collect()
            }
            Rule::UnderTheMean => {
                let average = mean(&values);
                let best = values
                    .iter()
                    .copied()
                    .filter(|value| *value < average - SAME)
                    .fold(f64::NEG_INFINITY, f64::max);
                if best == f64::NEG_INFINITY {
                    return Vec::new();
                }
                named
                    .iter()
                    .filter(|(_, value)| (value - best).abs() <= SAME)
                    .map(|(name, _)| *name)
                    .collect()
            }
            Rule::MostPopular => {
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for value in &values {
                    *counts.entry(*value as i64).or_default() += 1;
                }
                let most = counts.values().copied().max().unwrap_or(0);
                if most < 2 {
                    return Vec::new(); // no two people agreed on anything
                }
                let agreed: Vec<i64> = counts
                    .iter()
                    .filter(|(_, count)| **count == most)
                    .map(|(value, _)| *value)
                    .collect();
                named
                    .iter()
                    .filter(|(_, value)| agreed.contains(&(*value as i64)))
                    .map(|(name, _)| *name)
                    .collect()
            }
            Rule::Loneliest => {
                if values.len() < 2 {
                    return Vec::new(); // nobody to be far from
                }
                let loneliness = |mine: f64| {
                    values
                        .iter()
                        .enumerate()
                        .filter(|(_, other)| **other != mine)
                        .map(|(_, other)| (other - mine).abs())
                        .fold(f64::INFINITY, f64::min)
                };
                // Everybody who picked the same number is nought from
                // somebody, so a room where everyone agrees has no winner.
                let furthest = named
                    .iter()
                    .map(|(_, bound)| loneliness(*bound))
                    .filter(|d| d.is_finite())
                    .fold(0.0_f64, f64::max);
                if furthest <= 0.0 {
                    return Vec::new();
                }
                named
                    .iter()
                    .filter(|(_, value)| (loneliness(*value) - furthest).abs() <= SAME)
                    .map(|(name, _)| *name)
                    .collect()
            }
            Rule::UnderTheLimit { limit } => {
                if values.iter().sum::<f64>() > limit + SAME {
                    return Vec::new(); // the room asked for too much
                }
                let best = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                named
                    .iter()
                    .filter(|(_, value)| (value - best).abs() <= SAME)
                    .map(|(name, _)| *name)
                    .collect()
            }
            Rule::LowestUnique | Rule::HighestUnique => {
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for value in &values {
                    *counts.entry(*value as i64).or_default() += 1;
                }
                let alone: Vec<i64> = counts
                    .iter()
                    .filter(|(_, count)| **count == 1)
                    .map(|(value, _)| *value)
                    .collect();
                let wanted = if self.rule == Rule::LowestUnique {
                    alone.iter().min()
                } else {
                    alone.iter().max()
                };
                let Some(wanted) = wanted else {
                    return Vec::new();
                };
                named
                    .iter()
                    .filter(|(_, value)| *value as i64 == *wanted)
                    .map(|(name, _)| *name)
                    .collect()
            }
        }
    }

    fn target(&self, values: &[f64]) -> Option<f64> {
        if values.is_empty() {
            return None;
        }
        match self.rule {
            Rule::OfTheMean { num, den } => Some(mean(values) * num as f64 / den as f64),
            Rule::OfTheMedian { num, den } => Some(median(values) * num as f64 / den as f64),
            Rule::UnderTheMean => Some(mean(values)),
            // There is no number to aim at in any of these: what wins
            // depends on the shape of the whole room, not on one number.
            Rule::LowestUnique
            | Rule::HighestUnique
            | Rule::MostPopular
            | Rule::Loneliest
            | Rule::UnderTheLimit { .. } => None,
        }
    }

    /// The round's numbers, or `None` while it is still being played.
    pub fn outcome(&self, players: &[Player]) -> Option<Outcome> {
        if !self.revealed {
            return None;
        }
        let named = self.named(players);
        if named.is_empty() {
            return None;
        }
        let values: Vec<f64> = named.iter().map(|(_, value)| *value).collect();
        let winners = self.winners(&named);
        let mut picks: Vec<Chosen> = named
            .iter()
            .map(|(name, value)| Chosen {
                name: (*name).to_string(),
                value: *value,
                won: winners.contains(name),
            })
            .collect();
        picks.sort_by(|a, b| a.value.total_cmp(&b.value));
        Some(Outcome {
            n: named.len(),
            mean: mean(&values),
            median: median(&values),
            target: self.target(&values),
            label: self.rule.label(),
            winners: winners.iter().map(|name| (*name).to_string()).collect(),
            picks,
        })
    }
}

/// How one player's evening has gone.
#[derive(Serialize)]
pub struct Score {
    pub name: String,
    pub played: usize,
    pub wins: usize,
    /// What they pick on average, which is the number that moves as a room
    /// works a rule out.
    pub average: Option<f64>,
}

impl Game {
    /// The evening so far, most wins first, over the revealed rounds only —
    /// a tally including the round being played would give away who is
    /// winning it.
    pub fn scores(&self) -> Vec<Score> {
        let revealed: Vec<&Round> = self.rounds.iter().filter(|round| round.revealed).collect();
        let mut scores: Vec<Score> = self
            .players
            .iter()
            .map(|player| {
                let picks: Vec<f64> = revealed
                    .iter()
                    .filter_map(|round| round.picks.get(&player.key).map(|pick| pick.value))
                    .collect();
                let wins = revealed
                    .iter()
                    .filter(|round| {
                        round
                            .outcome(&self.players)
                            .is_some_and(|outcome| outcome.winners.contains(&player.name))
                    })
                    .count();
                Score {
                    name: player.name.clone(),
                    played: picks.len(),
                    wins,
                    average: (!picks.is_empty()).then(|| mean(&picks)),
                }
            })
            .filter(|score| score.played > 0)
            .collect();
        scores.sort_by(|a, b| b.wins.cmp(&a.wins).then(a.name.cmp(&b.name)));
        scores
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THIRD: Rule = Rule::OfTheMean { num: 1, den: 3 };

    fn room(rule: Rule, low: f64, high: f64, picks: &[f64]) -> Game {
        let mut game = Game::new(1000);
        for (i, value) in picks.iter().enumerate() {
            let name = ["Ada", "Bo", "Cy", "Di", "Eve", "Fay"][i];
            let key = game.join(name, 1000).unwrap();
            if game.rounds.is_empty() {
                game.start(rule, low, high, 1000).unwrap();
            }
            game.pick(&key, *value, 1001).unwrap();
        }
        game
    }

    fn outcome(game: &Game) -> Outcome {
        game.round()
            .unwrap()
            .outcome(&game.players)
            .expect("numbers")
    }

    #[test]
    fn a_third_of_the_average_is_what_everyone_is_aiming_at() {
        let mut game = room(THIRD, 0.0, 10.0, &[9.0, 6.0, 3.0, 0.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.mean, 4.5);
        assert_eq!(out.target, Some(1.5));
        assert_eq!(out.winners, vec!["Cy", "Di"], "3 and 0 are both 1.5 off");
        assert_eq!(out.picks.len(), 4);
        assert_eq!(out.picks[0].value, 0.0, "lowest first");
        assert!(out.picks[0].won);
    }

    #[test]
    fn two_people_the_same_distance_away_both_win() {
        let mut game = room(THIRD, 0.0, 10.0, &[9.0, 0.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.mean, 4.5);
        assert_eq!(out.target, Some(1.5));
        assert_eq!(out.winners, vec!["Bo"], "0 is 1.5 off and 9 is 7.5 off");
    }

    #[test]
    fn a_tie_is_shared() {
        let mut game = room(Rule::OfTheMean { num: 1, den: 1 }, 0.0, 10.0, &[2.0, 4.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.target, Some(3.0));
        assert_eq!(out.winners, vec!["Ada", "Bo"], "both are one away");
    }

    #[test]
    fn the_middle_number_can_be_aimed_at_instead_of_the_average() {
        let rule = Rule::OfTheMedian { num: 2, den: 3 };
        let mut game = room(rule, 0.0, 100.0, &[0.0, 3.0, 99.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.median, 3.0, "the 99 does not drag the middle");
        assert_eq!(out.target, Some(2.0));
        assert_eq!(out.winners, vec!["Bo"]);
    }

    #[test]
    fn the_lowest_number_nobody_else_picked_wins() {
        let mut game = room(Rule::LowestUnique, 1.0, 10.0, &[1.0, 1.0, 2.0, 3.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.target, None, "there is no number to aim at");
        assert_eq!(out.winners, vec!["Cy"], "1 was picked twice, so 2 wins");
    }

    #[test]
    fn nobody_wins_when_every_number_is_a_duplicate() {
        let mut game = room(Rule::LowestUnique, 1.0, 10.0, &[4.0, 4.0, 7.0, 7.0]);
        game.reveal().unwrap();
        assert!(outcome(&game).winners.is_empty());
    }

    #[test]
    fn the_highest_unique_number_is_the_other_way_round() {
        let mut game = room(Rule::HighestUnique, 1.0, 10.0, &[9.0, 9.0, 8.0, 1.0]);
        game.reveal().unwrap();
        assert_eq!(outcome(&game).winners, vec!["Cy"], "9 was taken twice");
    }

    #[test]
    fn under_the_average_goes_to_the_highest_one_that_is_still_under() {
        let mut game = room(Rule::UnderTheMean, 0.0, 100.0, &[10.0, 20.0, 90.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.target, Some(40.0));
        assert_eq!(out.winners, vec!["Bo"], "20 is the highest below 40");
    }

    #[test]
    fn nobody_is_under_the_average_when_everybody_picks_the_same() {
        let mut game = room(Rule::UnderTheMean, 0.0, 100.0, &[5.0, 5.0, 5.0]);
        game.reveal().unwrap();
        assert!(outcome(&game).winners.is_empty());
    }

    #[test]
    fn the_number_the_most_people_picked_wins() {
        let mut game = room(Rule::MostPopular, 1.0, 10.0, &[7.0, 7.0, 3.0, 9.0]);
        game.reveal().unwrap();
        let out = outcome(&game);
        assert_eq!(out.target, None);
        assert_eq!(out.winners, vec!["Ada", "Bo"], "two of them said 7");
    }

    #[test]
    fn nobody_wins_the_popular_one_when_no_two_people_agree() {
        let mut game = room(Rule::MostPopular, 1.0, 10.0, &[1.0, 2.0, 3.0]);
        game.reveal().unwrap();
        assert!(outcome(&game).winners.is_empty());
    }

    #[test]
    fn the_loneliest_number_is_the_one_furthest_from_the_rest() {
        let mut game = room(Rule::Loneliest, 0.0, 100.0, &[10.0, 12.0, 14.0, 90.0]);
        game.reveal().unwrap();
        assert_eq!(outcome(&game).winners, vec!["Di"], "76 from the nearest");
    }

    #[test]
    fn nobody_is_lonely_when_everybody_picked_the_same_number() {
        let mut game = room(Rule::Loneliest, 0.0, 100.0, &[5.0, 5.0, 5.0]);
        game.reveal().unwrap();
        assert!(outcome(&game).winners.is_empty());
        let mut game = room(Rule::Loneliest, 0.0, 100.0, &[5.0]);
        game.reveal().unwrap();
        assert!(outcome(&game).winners.is_empty(), "nobody to be far from");
    }

    #[test]
    fn the_highest_wins_under_the_limit_and_nobody_wins_over_it() {
        let rule = Rule::UnderTheLimit { limit: 100.0 };
        let mut game = room(rule, 0.0, 100.0, &[10.0, 20.0, 30.0]);
        game.reveal().unwrap();
        assert_eq!(outcome(&game).winners, vec!["Cy"], "60 is under 100");

        let mut game = room(rule, 0.0, 100.0, &[10.0, 20.0, 80.0]);
        game.reveal().unwrap();
        assert!(
            outcome(&game).winners.is_empty(),
            "110 is over the limit, so the room gets nothing"
        );
    }

    #[test]
    fn a_limit_has_to_be_a_number_above_nought() {
        let mut game = Game::new(1000);
        assert!(
            game.start(Rule::UnderTheLimit { limit: 0.0 }, 0.0, 10.0, 1000)
                .is_err()
        );
        assert!(
            game.start(Rule::UnderTheLimit { limit: 50.0 }, 0.0, 10.0, 1000)
                .is_ok()
        );
    }

    #[test]
    fn nothing_comes_out_of_a_round_that_has_not_been_revealed() {
        let game = room(THIRD, 0.0, 10.0, &[1.0, 2.0]);
        assert!(game.round().unwrap().outcome(&game.players).is_none());
    }

    #[test]
    fn a_pick_outside_the_range_is_refused() {
        let mut game = room(THIRD, 0.0, 10.0, &[5.0]);
        let key = game.players[0].key.clone();
        assert!(game.pick(&key, 11.0, 1002).is_err());
        assert!(game.pick(&key, -1.0, 1002).is_err());
        assert!(game.pick(&key, 10.0, 1002).is_ok(), "the ends are in");
    }

    #[test]
    fn the_unique_rules_are_played_with_whole_numbers() {
        let mut game = room(Rule::LowestUnique, 1.0, 10.0, &[3.0]);
        let key = game.players[0].key.clone();
        assert!(game.pick(&key, 2.5, 1002).is_err());
        let mut game = Game::new(1000);
        assert!(game.start(Rule::LowestUnique, 0.5, 10.0, 1000).is_err());
    }

    #[test]
    fn a_round_that_has_been_revealed_takes_no_more_picks() {
        let mut game = room(THIRD, 0.0, 10.0, &[5.0]);
        game.reveal().unwrap();
        let key = game.players[0].key.clone();
        assert!(game.pick(&key, 1.0, 1002).is_err());
        assert!(game.set_open(true).is_err(), "and cannot be reopened");
    }

    #[test]
    fn a_fraction_has_to_be_one_of_the_ones_on_offer() {
        let mut game = Game::new(1000);
        assert!(
            game.start(Rule::OfTheMean { num: 0, den: 3 }, 0.0, 10.0, 1000)
                .is_err()
        );
        assert!(
            game.start(Rule::OfTheMean { num: 4, den: 3 }, 0.0, 10.0, 1000)
                .is_err()
        );
        assert!(
            game.start(Rule::OfTheMean { num: 1, den: 0 }, 0.0, 10.0, 1000)
                .is_err()
        );
        assert!(game.start(THIRD, 0.0, 10.0, 1000).is_ok());
    }

    #[test]
    fn a_range_has_to_go_upwards() {
        let mut game = Game::new(1000);
        assert!(game.start(THIRD, 10.0, 0.0, 1000).is_err());
        assert!(game.start(THIRD, 5.0, 5.0, 1000).is_err());
    }

    #[test]
    fn the_scoreboard_counts_the_revealed_rounds_only() {
        let mut game = room(THIRD, 0.0, 10.0, &[9.0, 0.0]);
        let keys: Vec<String> = game.players.iter().map(|p| p.key.clone()).collect();
        game.reveal().unwrap();
        game.start(THIRD, 0.0, 10.0, 2000).unwrap();
        game.pick(&keys[0], 1.0, 2001).unwrap();
        game.pick(&keys[1], 8.0, 2001).unwrap();

        let scores = game.scores();
        assert_eq!(scores[0].name, "Bo", "Bo won the revealed round");
        assert_eq!(scores[0].wins, 1);
        assert_eq!(scores[0].played, 1, "the round still going is not counted");
        assert_eq!(scores[0].average, Some(0.0));
        assert_eq!(scores[1].wins, 0);
    }

    #[test]
    fn every_rule_says_what_it_is() {
        for (rule, label) in [
            (THIRD, "closest to 1/3 of the average"),
            (Rule::OfTheMean { num: 1, den: 1 }, "closest to the average"),
            (
                Rule::OfTheMedian { num: 2, den: 3 },
                "closest to 2/3 of the middle number",
            ),
            (Rule::LowestUnique, "the lowest number nobody else picked"),
            (Rule::HighestUnique, "the highest number nobody else picked"),
            (Rule::UnderTheMean, "the highest number below the average"),
            (Rule::MostPopular, "the number the most people picked"),
            (Rule::Loneliest, "the number furthest from everybody else's"),
            (
                Rule::UnderTheLimit { limit: 100.0 },
                "the highest number, if everybody's together come to no more than 100",
            ),
        ] {
            assert_eq!(rule.label(), label);
        }
    }
}
