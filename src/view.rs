//! What each screen is sent.
//!
//! Nobody is sent anybody else's number until the host reveals the round.
//! That includes the host, whose screen may still be projected. Until the
//! reveal, every screen gets the rule, the range and a count.
//!
//! The wall, the screen meant for a projector, never pairs a name with a
//! number: after the reveal it shows the numbers as unnamed dots, and
//! names only on the top of the scoreboard.

use rooms::lobby::Present;
use serde::Serialize;

use crate::answer::Answers;
use crate::game::{Game, Outcome, Round, Score};

#[derive(Serialize)]
pub struct Who<'a> {
    name: &'a str,
    picked: bool,
}

#[derive(Serialize)]
pub struct Playing {
    /// The rule in words, which is all a screen needs.
    label: String,
    low: f64,
    high: f64,
    /// Whether the round is played with whole numbers.
    whole: bool,
    /// What a pick can be typed as.
    answers: Answers,
    open: bool,
    revealed: bool,
    asked: u64,
}

#[derive(Serialize)]
pub struct Past {
    label: String,
    target: Option<f64>,
    mean: Option<f64>,
    winners: Vec<String>,
    n: usize,
}

#[derive(Serialize)]
pub struct Host<'a> {
    join: &'a str,
    /// The wall's key, for "Open the wall view".
    wall: &'a str,
    /// Everybody, with whose phone is connected.
    people: Vec<Present<'a>>,
    /// Everybody, with whether they have picked in this round.
    players: Vec<Who<'a>>,
    round: Option<Playing>,
    /// Only after the round has been revealed.
    outcome: Option<Outcome>,
    /// Newest first, excluding the round being played.
    history: Vec<Past>,
    scores: Vec<Score>,
}

#[derive(Serialize)]
pub struct Player<'a> {
    name: &'a str,
    round: Option<Playing>,
    /// This player's number, which they can change until the round
    /// closes.
    mine: Option<f64>,
    /// What this player typed, when the round takes expressions.
    mine_text: Option<String>,
    picked: usize,
    players: usize,
    outcome: Option<Outcome>,
    won: Option<bool>,
    score: Option<Score>,
}

impl Round {
    fn playing(&self) -> Playing {
        Playing {
            label: self.rule.label(),
            low: self.low,
            high: self.high,
            whole: self.whole(),
            answers: self.answers(),
            open: self.open,
            revealed: self.revealed,
            asked: self.asked,
        }
    }
}

fn history(game: &Game) -> Vec<Past> {
    game.rounds
        .iter()
        .rev()
        .skip(1)
        .filter_map(|round| {
            let outcome = round.outcome(game.players())?;
            Some(Past {
                label: outcome.label,
                target: outcome.target,
                mean: outcome.mean,
                winners: outcome.winners,
                n: outcome.n,
            })
        })
        .collect()
}

pub fn host(game: &Game, now: u64) -> Host<'_> {
    let round = game.round();
    Host {
        join: &game.lobby.join,
        wall: &game.lobby.wall,
        people: game.lobby.roster(now),
        players: game
            .players()
            .iter()
            .map(|player| Who {
                name: &player.name,
                picked: round.is_some_and(|r| r.picks.contains_key(&player.key)),
            })
            .collect(),
        round: round.map(Round::playing),
        outcome: round.and_then(|round| round.outcome(game.players())),
        history: history(game),
        scores: game.scores(),
    }
}

pub fn player<'a>(game: &'a Game, key: &str) -> Option<Player<'a>> {
    let player = game.player(key)?;
    let round = game.round();
    let outcome = round.and_then(|round| round.outcome(game.players()));
    Some(Player {
        name: &player.name,
        round: round.map(Round::playing),
        mine: round.and_then(|r| r.picks.get(key)).map(|pick| pick.value),
        mine_text: round
            .and_then(|r| r.picks.get(key))
            .and_then(|pick| pick.text.clone()),
        picked: round.map_or(0, |r| r.picks.len()),
        players: game.lobby.len(),
        won: outcome
            .as_ref()
            .map(|outcome| outcome.winners.contains(&player.name)),
        outcome,
        score: game
            .scores()
            .into_iter()
            .find(|score| score.name == player.name),
    })
}

/// One number on the wall's strip after the reveal: how many picked it,
/// and not who.
#[derive(Serialize)]
pub struct Spot {
    value: f64,
    count: usize,
    won: bool,
}

/// What the wall shows of a round once it has come out.
#[derive(Serialize)]
pub struct Shown {
    n: usize,
    target: Option<f64>,
    target_label: Option<&'static str>,
    mean: Option<f64>,
    median: Option<f64>,
    total: Option<f64>,
    /// How many won, without their names: their names would say what
    /// they picked.
    winners: usize,
    why: Option<String>,
    spots: Vec<Spot>,
}

/// Somebody on the top of the scoreboard. Wins and rounds, and no
/// average: after one round, a player's average is their number.
#[derive(Serialize)]
pub struct Top {
    name: String,
    wins: usize,
    played: usize,
}

/// The game's part of the wall. See [`crate::game::Game`]'s `HasLobby`.
#[derive(Serialize)]
pub struct Wall {
    round: Option<Playing>,
    /// How many of the people in the room have picked in this round.
    picked: usize,
    players: usize,
    /// Only after the reveal.
    shown: Option<Shown>,
    /// The three with the most wins, once anybody has won.
    top: Vec<Top>,
}

pub fn wall(game: &Game) -> Wall {
    let round = game.round();
    let shown = round
        .and_then(|round| round.outcome(game.players()))
        .map(|outcome| {
            let mut spots: Vec<Spot> = Vec::new();
            for pick in &outcome.picks {
                match spots.iter_mut().find(|spot| spot.value == pick.value) {
                    Some(spot) => {
                        spot.count += 1;
                        spot.won |= pick.won;
                    }
                    None => spots.push(Spot {
                        value: pick.value,
                        count: 1,
                        won: pick.won,
                    }),
                }
            }
            Shown {
                n: outcome.n,
                target: outcome.target,
                target_label: outcome.target_label,
                mean: outcome.mean,
                median: outcome.median,
                total: outcome.total,
                winners: outcome.winners.len(),
                why: outcome.why,
                spots,
            }
        });
    Wall {
        round: round.map(Round::playing),
        picked: round.map_or(0, |round| {
            game.players()
                .iter()
                .filter(|p| round.picks.contains_key(&p.key))
                .count()
        }),
        players: game.lobby.len(),
        shown,
        top: game
            .scores()
            .into_iter()
            .filter(|score| score.wins > 0)
            .take(3)
            .map(|score| Top {
                name: score.name,
                wins: score.wins,
                played: score.played,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Rule;
    use rooms::lobby::admit;

    const THIRD: Rule = Rule::OfTheMean { num: 1, den: 3 };

    fn room() -> (Game, String, String) {
        let mut game = Game::new(1000);
        let ada = admit(&mut game, "Ada", None, 1000).unwrap();
        let bo = admit(&mut game, "Bo", None, 1000).unwrap();
        game.start(THIRD, 0.0, 10.0, 1000).unwrap();
        game.pick(&ada, 9.0, 1001).unwrap();
        game.pick(&bo, 1.0, 1001).unwrap();
        (game, ada, bo)
    }

    fn json<T: Serialize>(value: &T) -> String {
        serde_json::to_string(value).expect("the view serializes")
    }

    #[test]
    fn nobody_sees_a_number_before_the_reveal_not_even_the_host() {
        let (mut game, ada, _) = room();
        let seen = json(&host(&game, 1000));
        assert!(
            !seen.contains("\"value\""),
            "the host screen is on the wall"
        );
        assert!(
            json(&player(&game, &ada).unwrap()).contains("\"mine\":9"),
            "a player still sees their own"
        );

        game.reveal().unwrap();
        assert!(json(&host(&game, 1000)).contains("\"value\":9"));
    }

    #[test]
    fn the_count_is_all_anyone_gets_until_then() {
        let (game, ada, _) = room();
        let view = player(&game, &ada).unwrap();
        assert_eq!((view.picked, view.players), (2, 2));
        assert!(view.outcome.is_none());
        assert!(view.won.is_none());
        let view = host(&game, 1000);
        assert!(view.players.iter().all(|who| who.picked));
        assert!(view.outcome.is_none());
    }

    #[test]
    fn the_numbers_and_the_winner_come_out_together() {
        let (mut game, ada, bo) = room();
        game.reveal().unwrap();
        let view = player(&game, &bo).unwrap();
        let outcome = view.outcome.expect("the round came out");
        assert_eq!(outcome.mean, Some(5.0));
        assert_eq!(outcome.target, Some(5.0 / 3.0));
        assert_eq!(outcome.winners, vec!["Bo"]);
        assert_eq!(view.won, Some(true));
        assert_eq!(player(&game, &ada).unwrap().won, Some(false));
    }

    #[test]
    fn a_round_still_being_played_is_not_in_the_history() {
        let (mut game, _, _) = room();
        game.reveal().unwrap();
        game.start(THIRD, 0.0, 10.0, 2000).unwrap();
        let view = host(&game, 1000);
        assert_eq!(view.history.len(), 1);
        assert_eq!(view.history[0].winners, vec!["Bo"]);
        assert!(view.outcome.is_none(), "the new round has not come out");
    }

    #[test]
    fn a_room_with_no_round_in_it_still_renders() {
        let mut game = Game::new(1000);
        let key = admit(&mut game, "Ada", None, 1000).unwrap();
        assert!(host(&game, 1000).round.is_none());
        let view = player(&game, &key).unwrap();
        assert!(view.round.is_none());
        assert!(view.mine.is_none());
    }

    #[test]
    fn a_key_that_is_not_in_the_room_has_no_view() {
        let (game, _, _) = room();
        assert!(player(&game, "nobody").is_none());
    }
}
