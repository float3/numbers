# numbers

[![CI](https://github.com/float3/numbers/actions/workflows/ci.yml/badge.svg)](https://github.com/float3/numbers/actions/workflows/ci.yml)

Number games for a room of people, at
[numbers.rationality-munich.com](https://numbers.rationality-munich.com).

Everyone picks a number, and the rule decides who wins. The rules all have
the same shape: what your number is worth depends on what everybody else
picked, so playing well means working out what the room will do — and what
the room thinks the room will do.

Built on [rooms](https://github.com/float3/rooms).

## The rules

| | |
| --- | --- |
| Closest to a fraction of the average | 1/3, 1/2, 2/3, or the average itself |
| Closest to a fraction of the middle number | the same game, where one silly answer cannot drag the target |
| The lowest number nobody else picked | whole numbers; some rounds have no winner |
| The highest number nobody else picked | |
| The highest number below the average | going high wins until everybody goes high |

The first one is the old Keynesian beauty contest. At a third of the
average, the average cannot be more than a third of itself unless somebody
is wrong, so the only number the whole room can hold is nought — and no
room starts there. The interesting part is the second round.

## Nobody sees a number until the reveal

Including the host, because the host's screen is often the one on the wall.
Until the host reveals the round, every screen gets the rule, the range,
and a count of who has picked. Then every number appears at once, with the
winner, the average, the middle number and the number the room was aiming
at.

## Three links

| | |
| --- | --- |
| `/h/<key>` | runs the room. Keep it; it is the only way back in. |
| `/j/<key>` | joins it. This is the one on the wall. |
| `/p/<key>` | one player's own, kept by their phone. |

The key in the link is the whole access model: 24 random characters and no
accounts. See [rooms](https://github.com/float3/rooms) for what that is and
is not good for.

## Running it

```sh
nix run github:float3/numbers
```

| | |
| --- | --- |
| `NUMBERS_ADDR` | what to listen on. `127.0.0.1:8098`. |
| `NUMBERS_STATE` | where the rooms are kept. `rooms.json`. |

The NixOS module takes a port and the domains to serve on:

```nix
services.numbers = {
  enable = true;
  port = 8098;
  domains = ["numbers.example.com"];
};
```

Rooms are forgotten a day after anyone last looked at one.

## Development

```sh
nix develop --command cargo test
```
