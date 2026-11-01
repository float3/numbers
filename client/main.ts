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
  joining,
  list,
  num,
  onClick,
  onSubmit,
  opening,
  plural,
  route,
  setValue,
  show,
  showJoin,
  text,
  value,
  watch,
} from "./rooms";
import { type Answers, read } from "./answer";

interface Playing {
  label: string;
  low: number;
  high: number;
  whole: boolean;
  answers: Answers;
  open: boolean;
  revealed: boolean;
  asked: number;
}

interface Chosen {
  name: string;
  value: number;
  text: string | null;
  won: boolean;
}

interface Outcome {
  n: number;
  mean: number | null;
  median: number | null;
  target: number | null;
  target_label: string | null;
  total: number | null;
  label: string;
  winners: string[];
  why: string | null;
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
  mean: number | null;
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
  mine_text: string | null;
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
  /** Rules that count matching picks, which need whole numbers. */
  whole?: true;
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
    whole: true,
    low: 1,
    high: 20,
  },
  {
    id: "highest-unique",
    name: "The highest number nobody else picked",
    rule: "highest-unique",
    whole: true,
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
    whole: true,
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

/** A pick as it was typed, with what it came to when that differs. */
const typed = (text: string | null, value: number): string =>
  text === null || text === num(value) ? num(value) : `${text} = ${num(value)}`;

const ANSWERS: Record<Answers, string> = {
  whole: "whole numbers",
  decimal: "decimals allowed",
  expression: "sums allowed, like 1,001/3",
};

/** Every number along the round's range, with the target marked. */
function strip(round: Playing, outcome: Outcome): string {
  const width = round.high - round.low || 1;
  const at = (given: number) =>
    Math.min(100, Math.max(0, ((given - round.low) / width) * 100));
  const target =
    outcome.target === null
      ? ""
      : `<div class="target" style="left:${at(outcome.target)}%"><span>${num(outcome.target)}</span></div>`;
  // One dot per distinct number, sized by how many picked it, with the
  // winning numbers drawn last so nothing covers them.
  const spots = new Map<number, { value: number; names: string[]; won: boolean }>();
  for (const pick of outcome.picks) {
    const spot = spots.get(pick.value) ?? { value: pick.value, names: [], won: false };
    spot.names.push(pick.name);
    spot.won ||= pick.won;
    spots.set(pick.value, spot);
  }
  const dots = [...spots.values()]
    .sort((a, b) => Number(a.won) - Number(b.won))
    .map(
      (spot) =>
        `<div class="pick${spot.won ? " winner" : ""}" style="left:${at(spot.value)}%" title="${esc(list(spot.names))}: ${num(spot.value)}">${
          spot.names.length > 1 ? `<i>${spot.names.length}</i>` : ""
        }${spot.won ? `<span>${esc(list(spot.names))}</span>` : ""}</div>`,
    )
    .join("");
  return `<div class="strip">${target}${dots}</div>
    <div class="ends"><span>${num(round.low)}</span><span>${num(round.high)}</span></div>`;
}

function outcomeHtml(round: Playing, outcome: Outcome): string {
  const winners =
    outcome.winners.length > 0
      ? `<div class="verdict won"><p class="eyebrow">${outcome.winners.length === 1 ? "The winner" : "Shared between"}</p>
         <p class="big">${esc(list(outcome.winners))}</p></div>`
      : `<div class="verdict none"><p class="eyebrow">Nobody won</p>
         <p class="big">${esc(outcome.why ?? "Nobody's number met the rule this round.")}</p></div>`;
  if (outcome.n === 0) return winners;
  const rows: [string, string][] = [
    ["The average", num(outcome.mean)],
    ["The middle number", num(outcome.median)],
  ];
  if (outcome.target !== null && outcome.target_label !== "The average, to stay under") {
    rows.unshift([outcome.target_label ?? "The number to aim at", num(outcome.target)]);
  }
  if (outcome.total !== null) {
    rows.unshift(["Everybody together", num(outcome.total)]);
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
        <td>${pick.won ? "🏆 " : ""}${esc(pick.name)}</td><td>${esc(typed(pick.text, pick.value))}</td>
      </tr>`,
        )
        .join("")}</tbody></table></div>`;
}


// ------------------------------------------------------------------- host

function host(key: string): void {
  show("host");
  const at = (path: string) => `/api/h/${encodeURIComponent(key)}${path}`;
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
  const answers = byId<HTMLSelectElement>("answers");
  answers.value = "decimal";
  function answersNote(): void {
    const rule = chosen();
    text(
      "answers-note",
      rule.whole
        ? "This rule counts who picked the same number, so it needs whole numbers. Expressions are fine if they come out whole."
        : answers.value === "expression"
          ? "Phones show what a sum comes to before it is sent, and everybody sees the sums at the reveal."
          : "",
    );
  }
  answers.addEventListener("change", answersNote);
  function ruleChanged(): void {
    const rule = chosen();
    const decimal = answers.querySelector<HTMLOptionElement>('option[value="decimal"]');
    if (decimal) decimal.disabled = rule.whole === true;
    if (rule.whole && answers.value === "decimal") answers.value = "whole";
    answersNote();
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
      answers: answers.value,
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
    showJoin(view.join);
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
        <span>${past.n === 0 ? "nobody picked" : `${past.target === null ? "" : `target ${num(past.target)} · `}average ${num(past.mean)} · ${
          past.winners.length > 0 ? `won by ${esc(list(past.winners))}` : "nobody won"
        }`}</span>
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
        `${num(round.low)} to ${num(round.high)}, ${ANSWERS[round.answers]}`,
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

  // Fetches the room once, then keeps it up to date from its event
  // stream; `watch` adds the "/events" itself.
  watch<HostView>(at(""), draw);
}


// ----------------------------------------------------------------- player

function player(key: string): void {
  show("player");
  const at = (path: string) => `/api/p/${encodeURIComponent(key)}${path}`;
  const field = byId<HTMLInputElement>("pick");
  let filledFor: number | null = null;
  let last: PlayerView | null = null;
  let sending = false;

  /** What the field reads as, in the current round. */
  const reading = () => (last?.round ? read(last.round.answers, field.value) : null);

  /** Whether what is in the field is what the room already has. */
  function unchanged(view: PlayerView): boolean {
    if (view.mine === null) return false;
    if (view.mine_text !== null) return field.value.trim() === view.mine_text;
    const r = reading();
    return r !== null && "value" in r && r.value === view.mine;
  }

  /** The line under the field: what it reads as, or why it does not. */
  function drawReading(): void {
    const round = last?.round;
    const line = byId("reading");
    const r = reading();
    if (!round || r === null || field.value.trim() === "") {
      line.textContent = "";
      line.className = "reading";
      return;
    }
    if ("error" in r) {
      line.textContent = r.error;
      line.className = "reading bad";
    } else if (r.value < round.low || r.value > round.high) {
      line.textContent = `That comes to ${num(r.value)}, outside ${num(round.low)} to ${num(round.high)}.`;
      line.className = "reading bad";
    } else {
      // Only worth saying when it is not just the number typed back.
      const plain = field.value.trim();
      line.textContent = plain === String(r.value) ? "" : `Reads as ${num(r.value)}`;
      line.className = "reading";
    }
  }

  /** Whether your number is in, and whether you can still change it. */
  function drawStatus(): void {
    const view = last;
    const round = view?.round;
    const status = byId("status");
    const send = byId<HTMLButtonElement>("send");
    if (!view || !round || round.revealed) {
      hide("status", true);
      return;
    }
    hide("status", false);
    const yours = view.mine === null ? "" : typed(view.mine_text, view.mine);
    if (!round.open) {
      status.className = `status ${view.mine === null ? "missed" : "locked"}`;
      status.innerHTML =
        view.mine === null
          ? `<b>You did not send a number</b><span>The host has stopped taking numbers.</span>`
          : `<b>🔒 Locked in: ${esc(yours)}</b><span>The host has stopped taking numbers, so this is final. Wait for the reveal.</span>`;
      return;
    }
    if (view.mine === null) {
      status.className = "status waiting";
      status.innerHTML = `<b>Not sent yet</b><span>Your number only counts once you press Send.</span>`;
      send.textContent = "Send";
      send.disabled = sending;
    } else if (unchanged(view)) {
      status.className = "status sent";
      status.innerHTML = `<b>✓ Sent: ${esc(yours)}</b><span>You can still change it until the host stops taking numbers.</span>`;
      send.textContent = "Sent ✓";
      send.disabled = true;
    } else {
      status.className = "status changed";
      status.innerHTML = `<b>Your change is not sent yet</b><span>The room still has ${esc(yours)} from you. Press Send to replace it.</span>`;
      send.textContent = "Send the change";
      send.disabled = sending;
    }
    text("pick-label", view.mine === null ? "Your number" : "Change your number");
  }

  field.addEventListener("input", () => {
    drawReading();
    drawStatus();
  });

  onSubmit("pick-form", async () => {
    const r = reading();
    if (r === null || "error" in r) {
      fail(r?.error ?? "Pick a number first.");
      return;
    }
    sending = true;
    try {
      const body = await api<{ view: PlayerView }>("PUT", at("/pick"), {
        text: field.value.trim(),
      });
      draw(body.view);
      fail("");
      // A moment of movement, so a press that sent is not mistaken for
      // one that did nothing.
      const status = byId("status");
      status.classList.remove("just");
      void status.offsetWidth;
      status.classList.add("just");
    } finally {
      sending = false;
      drawStatus();
    }
  });

  function draw(view: PlayerView): void {
    last = view;
    html("you", `<span class="dot"></span> You are ${esc(view.name)}`);
    const round = view.round;
    if (!round) {
      text("p-state", "Waiting");
      text("p-rule", "Waiting for the first round");
      text("p-range", "");
      text("mine", "");
      hide("pick-form", true);
      hide("status", true);
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
      `Anything from ${num(round.low)} to ${num(round.high)}. ${
        round.answers === "whole"
          ? "Whole numbers only."
          : round.answers === "decimal"
            ? "Decimals are fine."
            : `You can type a sum, like 1,001/3 or 2^10 - 1${round.whole ? ", as long as it comes out whole" : ""}.`
      }`,
    );
    hide("pick-form", !round.open);

    // A phone keyboard to suit: digits for plain numbers, letters and
    // signs for sums, and a minus key whenever the range goes below nought.
    field.inputMode =
      round.answers === "expression" || round.low < 0 ? "text" : round.answers === "whole" ? "numeric" : "decimal";
    field.placeholder = round.answers === "expression" ? "e.g. 1,001/3" : "";
    if (filledFor !== round.asked) {
      filledFor = round.asked;
      field.value = view.mine === null ? "" : (view.mine_text ?? String(view.mine));
    }
    drawReading();
    drawStatus();

    text(
      "mine",
      round.revealed ? "" : `${view.picked} of ${plural(view.players, "person", "people")} have sent a number.`,
    );

    html(
      "p-outcome",
      view.outcome
        ? `<div class="panel">${
            view.mine === null
              ? `<p class="you-result">You did not pick this round.</p>`
              : view.won === true
                ? `<p class="you-result good">🎉 You won with ${esc(typed(view.mine_text, view.mine))}</p>`
                : `<p class="you-result">You picked ${esc(typed(view.mine_text, view.mine))}, not a winner this time.</p>`
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

  // Fetches the room once, then keeps it up to date from its event
  // stream; `watch` adds the "/events" itself.
  watch<PlayerView>(at(""), draw);
}

const where = route();
if (where.kind === "host") host(where.key);
else if (where.kind === "join") joining("numbers", where.key);
else if (where.kind === "player") player(where.key);
else opening();
