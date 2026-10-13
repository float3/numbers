// The number games, in a browser.
//
// One page with three screens: the one that opens a room, the host's, and
// the player's, which is almost always a phone.

import {
  api,
  byId,
  esc,
  fail,
  hide,
  html,
  joinLink,
  list,
  num,
  onClick,
  onSubmit,
  plural,
  qrInto,
  remember,
  remembered,
  route,
  setValue,
  show,
  text,
  value,
  watch,
} from "./rooms";

interface Playing {
  label: string;
  low: number;
  high: number;
  whole: boolean;
  open: boolean;
  revealed: boolean;
  asked: number;
}

interface Chosen {
  name: string;
  value: number;
  won: boolean;
}

interface Outcome {
  n: number;
  mean: number;
  median: number;
  target: number | null;
  label: string;
  winners: string[];
  picks: Chosen[];
}

interface Who {
  name: string;
  picked: boolean;
}

interface Score {
  name: string;
  played: number;
  wins: number;
  average: number | null;
}

interface Past {
  label: string;
  target: number | null;
  mean: number;
  winners: string[];
  n: number;
}

interface HostView {
  join: string;
  players: Who[];
  round: Playing | null;
  outcome: Outcome | null;
  history: Past[];
  scores: Score[];
}

interface PlayerView {
  name: string;
  round: Playing | null;
  mine: number | null;
  picked: number;
  players: number;
  outcome: Outcome | null;
  won: boolean | null;
  score: Score | null;
}

/** How a rule is sent, and the range to start the fields at. */
interface Preset {
  id: string;
  name: string;
  rule: string;
  num?: number;
  den?: number;
  /** Only the limit rule has one, and only it shows the field. */
  limit?: number;
  low: number;
  high: number;
}

const RULES: Preset[] = [
  {
    id: "mean-1-3",
    name: "Closest to 1/3 of the average",
    rule: "of-the-mean",
    num: 1,
    den: 3,
    low: 0,
    high: 10,
  },
  {
    id: "mean-2-3",
    name: "Closest to 2/3 of the average",
    rule: "of-the-mean",
    num: 2,
    den: 3,
    low: 0,
    high: 100,
  },
  {
    id: "mean-1-2",
    name: "Closest to 1/2 of the average",
    rule: "of-the-mean",
    num: 1,
    den: 2,
    low: 0,
    high: 100,
  },
  {
    id: "mean-1-1",
    name: "Closest to the average",
    rule: "of-the-mean",
    num: 1,
    den: 1,
    low: 0,
    high: 100,
  },
  {
    id: "median-2-3",
    name: "Closest to 2/3 of the middle number",
    rule: "of-the-median",
    num: 2,
    den: 3,
    low: 0,
    high: 100,
  },
  {
    id: "lowest-unique",
    name: "The lowest number nobody else picked",
    rule: "lowest-unique",
    low: 1,
    high: 20,
  },
  {
    id: "highest-unique",
    name: "The highest number nobody else picked",
    rule: "highest-unique",
    low: 1,
    high: 20,
  },
  {
    id: "under-the-mean",
    name: "The highest number below the average",
    rule: "under-the-mean",
    low: 0,
    high: 100,
  },
  {
    id: "most-popular",
    name: "The number the most people pick",
    rule: "most-popular",
    low: 1,
    high: 10,
  },
  {
    id: "loneliest",
    name: "The number furthest from everybody else's",
    rule: "loneliest",
    low: 0,
    high: 100,
  },
  {
    id: "under-the-limit",
    name: "The highest number, if the room's total stays under the limit",
    rule: "under-the-limit",
    limit: 100,
    low: 0,
    high: 100,
  },
];

const chosen = (): Preset =>
  RULES.find((rule) => rule.id === byId<HTMLSelectElement>("rule").value) ??
  RULES[0];

const sentence = (said: string): string =>
  said.charAt(0).toUpperCase() + said.slice(1);

/** Every number along the round's range, with the target marked. */
function strip(round: Playing, outcome: Outcome): string {
  const width = round.high - round.low || 1;
  const at = (given: number) =>
    Math.min(100, Math.max(0, ((given - round.low) / width) * 100));
  const target =
    outcome.target === null
      ? ""
      : `<div class="target" style="left:${at(outcome.target)}%"><span>${num(outcome.target)}</span></div>`;
  const dots = outcome.picks
    .map(
      (pick) =>
        `<div class="pick${pick.won ? " winner" : ""}" style="left:${at(pick.value)}%" title="${esc(pick.name)}: ${num(pick.value)}"></div>`,
    )
    .join("");
  return `<div class="strip">${target}${dots}</div>
    <div class="ends"><span>${num(round.low)}</span><span>${num(round.high)}</span></div>`;
}

function outcomeHtml(round: Playing, outcome: Outcome): string {
  const winners =
    outcome.winners.length > 0
      ? `<p class="eyebrow">${outcome.winners.length === 1 ? "The winner" : "The winners"}</p>
         <p class="big good">${esc(list(outcome.winners))}</p>`
      : `<p class="eyebrow">Nobody won</p>
         <p class="note">Nobody's number met the rule this round.</p>`;
  const rows: [string, string][] = [
    ["The average", num(outcome.mean)],
    ["The middle number", num(outcome.median)],
  ];
  if (outcome.target !== null) {
    rows.unshift(["The number to aim at", num(outcome.target)]);
  }
  return `${winners}
    ${strip(round, outcome)}
    <div class="facts">${rows
      .map(
        ([name, said]) =>
          `<div class="fact"><span>${name}</span><b>${said}</b></div>`,
      )
      .join("")}</div>
    <div class="scroll"><table><thead><tr><th>Who</th><th>Picked</th></tr></thead>
      <tbody>${outcome.picks
        .map(
          (pick) => `<tr class="${pick.won ? "good" : ""}">
        <td>${esc(pick.name)}${pick.won ? " ✓" : ""}</td><td>${num(pick.value)}</td>
      </tr>`,
        )
        .join("")}</tbody></table></div>`;
}

// ---------------------------------------------------------------- opening

function start(): void {
  show("start");
  onClick("open", async () => {
    byId<HTMLButtonElement>("open").disabled = true;
    try {
      const keys = await api<{ host: string }>("POST", "/api/rooms");
      location.href = `/h/${keys.host}`;
    } catch (e) {
      byId<HTMLButtonElement>("open").disabled = false;
      throw e;
    }
  });
}

// ------------------------------------------------------------------- host

function host(key: string): void {
  show("host");
  const at = (path: string) => `/api/h/${encodeURIComponent(key)}${path}`;
  let link = "";
  let now: HostView | null = null;

  text("where", "You are the host");
  html(
    "rule",
    RULES.map(
      (rule) => `<option value="${rule.id}">${esc(rule.name)}</option>`,
    ).join(""),
  );

  /** Each rule has a range to start from, until the host edits one. */
  let touched = false;
  for (const id of ["low", "high"]) {
    byId(id).addEventListener("input", () => {
      touched = true;
    });
  }
  function ruleChanged(): void {
    const rule = chosen();
    hide("limit-field", rule.limit === undefined);
    if (rule.limit !== undefined) setValue("limit", rule.limit);
    if (touched) return;
    setValue("low", rule.low);
    setValue("high", rule.high);
  }
  byId("rule").addEventListener("change", ruleChanged);
  ruleChanged();

  onSubmit("start-form", async () => {
    const rule = chosen();
    const body: Record<string, unknown> = {
      rule: rule.rule,
      low: value("low"),
      high: value("high"),
    };
    if (rule.num !== undefined) {
      body.num = rule.num;
      body.den = rule.den;
    }
    if (rule.limit !== undefined) body.limit = value("limit");
    await api("POST", at("/start"), body);
    fail("");
  });

  onClick("close", async () => {
    await api("PUT", at("/open"), { open: !now?.round?.open });
    fail("");
  });

  onClick("reveal", async () => {
    await api("POST", at("/reveal"));
    fail("");
  });

  function drawRoom(view: HostView): void {
    const next = joinLink(view.join);
    if (next !== link) {
      link = next;
      const anchor = byId<HTMLAnchorElement>("join-link");
      anchor.textContent = next;
      anchor.href = next;
      qrInto("join-qr", next);
    }
    byId("room").classList.toggle("tight", view.players.length > 0);
    html(
      "who",
      view.players.length === 0
        ? "Nobody has joined yet."
        : `<span class="dot"></span> ${esc(plural(view.players.length, "person", "people"))} in the room`,
    );
  }

  function drawBoard(view: HostView): void {
    if (view.scores.length === 0) {
      html("board", "");
      return;
    }
    html(
      "board",
      `<h3>The evening so far</h3><div class="scroll"><table><thead><tr>
        <th>Who</th><th>Rounds</th><th>Won</th><th>Picks on average</th>
      </tr></thead><tbody>${view.scores
        .map(
          (score) => `<tr>
        <td>${esc(score.name)}</td><td>${score.played}</td><td>${score.wins}</td>
        <td>${num(score.average)}</td>
      </tr>`,
        )
        .join("")}</tbody></table></div>`,
    );
  }

  function drawHistory(view: HostView): void {
    if (view.history.length === 0) {
      html("history", "");
      return;
    }
    html(
      "history",
      `<h3>Played already</h3>${view.history
        .map(
          (past) => `<div class="past">
        <span>${esc(sentence(past.label))}</span>
        <span>${past.target === null ? "" : `aim ${num(past.target)} · `}average ${num(past.mean)} · ${
          past.winners.length > 0 ? esc(list(past.winners)) : "nobody"
        }</span>
      </div>`,
        )
        .join("")}`,
    );
  }

  function draw(view: HostView): void {
    now = view;
    drawRoom(view);
    const round = view.round;
    hide("playing", round === null);
    if (round) {
      text(
        "h-state",
        round.revealed ? "Revealed" : round.open ? "Taking numbers" : "Closed",
      );
      text("h-rule", sentence(round.label));
      text(
        "h-range",
        `${num(round.low)} to ${num(round.high)}${round.whole ? ", whole numbers" : ""}`,
      );
      const picked = view.players.filter((who) => who.picked).length;
      const missing = view.players
        .filter((who) => !who.picked)
        .map((who) => who.name);
      text(
        "progress",
        `${picked} of ${plural(view.players.length, "person", "people")} picked` +
          (!round.revealed && missing.length > 0 && missing.length <= 6
            ? ` · waiting on ${list(missing)}`
            : "") +
          (round.revealed ? "" : " · nobody sees a number until you reveal"),
      );
      text("close", round.open ? "Stop taking numbers" : "Take more numbers");
      byId<HTMLButtonElement>("close").disabled = round.revealed;
      byId<HTMLButtonElement>("reveal").disabled = round.revealed;
      text("reveal", round.revealed ? "Revealed" : "Reveal");
      html("outcome", view.outcome ? outcomeHtml(round, view.outcome) : "");
    }
    drawBoard(view);
    drawHistory(view);
  }

  api<{ view: HostView }>("GET", at(""))
    .then((body) => draw(body.view))
    .catch((e: unknown) => fail(e instanceof Error ? e.message : String(e)));
  watch<HostView>(at("/events"), draw);
}

// ------------------------------------------------------------------- join

function join(key: string): void {
  show("join");
  const kept = remembered("numbers", key);
  if (kept) {
    location.replace(`/p/${kept}`);
    return;
  }

  api<{ players: number }>("GET", `/api/j/${encodeURIComponent(key)}`)
    .then((body) => {
      text(
        "join-count",
        body.players === 0
          ? "You are the first one in."
          : `${plural(body.players, "person", "people")} already in.`,
      );
    })
    .catch((e: unknown) => fail(e instanceof Error ? e.message : String(e)));

  onSubmit("name-form", async () => {
    const body = await api<{ player: string }>(
      "POST",
      `/api/j/${encodeURIComponent(key)}`,
      { name: byId<HTMLInputElement>("name").value },
    );
    remember("numbers", key, body.player);
    location.href = `/p/${body.player}`;
  });
}

// ----------------------------------------------------------------- player

function player(key: string): void {
  show("player");
  const at = (path: string) => `/api/p/${encodeURIComponent(key)}${path}`;
  let filledFor: number | null = null;

  onSubmit("pick-form", async () => {
    const picked = value("pick");
    if (picked === null) {
      fail("Pick a number first.");
      return;
    }
    const body = await api<{ view: PlayerView }>("PUT", at("/pick"), {
      value: picked,
    });
    draw(body.view);
    fail("");
  });

  function draw(view: PlayerView): void {
    html("you", `<span class="dot"></span> You are ${esc(view.name)}`);
    const round = view.round;
    if (!round) {
      text("p-state", "Waiting");
      text("p-rule", "Waiting for the first round");
      text("p-range", "");
      hide("pick-form", true);
      hide("p-closed", true);
      html("p-outcome", "");
      return;
    }

    text(
      "p-state",
      round.revealed ? "Revealed" : round.open ? "Pick a number" : "Closed",
    );
    text("p-rule", sentence(round.label));
    text(
      "p-range",
      `Anything from ${num(round.low)} to ${num(round.high)}${round.whole ? ", whole numbers only." : "."}`,
    );
    hide("pick-form", !round.open);
    hide("p-closed", round.open);
    if (!round.open) {
      text(
        "p-closed",
        view.mine === null
          ? "You did not pick this round."
          : `You picked ${num(view.mine)}.`,
      );
    }

    const field = byId<HTMLInputElement>("pick");
    field.min = String(round.low);
    field.max = String(round.high);
    field.step = round.whole ? "1" : "any";
    if (filledFor !== round.asked) {
      filledFor = round.asked;
      setValue("pick", view.mine);
    }

    text(
      "mine",
      view.mine === null
        ? `${view.picked} of ${plural(view.players, "person", "people")} have picked.`
        : `Sent: ${num(view.mine)}. You can change it until the host stops taking numbers.`,
    );

    html(
      "p-outcome",
      view.outcome
        ? `<div class="panel">${
            view.won === true
              ? `<p class="eyebrow">You won this one</p>`
              : view.won === false
                ? `<p class="eyebrow">You did not win this one</p>`
                : ""
          }${outcomeHtml(round, view.outcome)}${
            view.score && view.score.played
              ? `<p class="note">Your evening: ${view.score.wins} of
                 ${plural(view.score.played, "round", "rounds")} won, picking
                 ${num(view.score.average)} on average.</p>`
              : ""
          }</div>`
        : "",
    );
  }

  api<{ view: PlayerView }>("GET", at(""))
    .then((body) => draw(body.view))
    .catch((e: unknown) => fail(e instanceof Error ? e.message : String(e)));
  watch<PlayerView>(at("/events"), draw);
}

const where = route();
if (where.kind === "host") host(where.key);
else if (where.kind === "join") join(where.key);
else if (where.kind === "player") player(where.key);
else start();
