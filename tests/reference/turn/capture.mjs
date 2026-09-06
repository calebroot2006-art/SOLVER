// Independent reference-process driver for turn-start trees. Imports no application module.
//
// Differences from the river driver (tests/reference/river/capture.mjs):
//   * the board is four cards, so the pinned binding builds a turn tree with one chance node
//     per live continuation, and a history entry at a chance node is a card ID, not an
//     action index;
//   * only the runouts named in each case's `export_runouts` are walked, because the full
//     48-runout export is hundreds of megabytes with real ranges. The solve, the
//     exploitability and the stop reason always cover the whole tree;
//   * decision nodes export the acting player's rows only; chance nodes export both players'
//     reach, normalized weights and expected values, which is what the turn-round per-combo
//     review uses as its continuation value.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

export const CAPTURE_VERSION = 1;
export const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;
export const MAX_NODES = 4000;
const RANKS = "23456789TJQKA";
const SUITS = "cdhs";

export function cardId(label) {
  assert.match(label, /^[2-9TJQKA][cdhs]$/);
  return 4 * RANKS.indexOf(label[0]) + SUITS.indexOf(label[1]);
}

export function cardLabel(id) {
  assert(Number.isInteger(id) && id >= 0 && id < 52);
  return RANKS[Math.floor(id / 4)] + SUITS[id % 4];
}

// crates/cards/src/range.rs grammar, reimplemented so the capture never depends on the
// external range parser. The two differ on token order and on `+` for non-pairs; see
// README.md, "Range strings".
function parseClass(text) {
  assert(text.length === 2 || text.length === 3, `expected a two-rank class: ${text}`);
  const high = RANKS.indexOf(text[0]);
  const low = RANKS.indexOf(text[1]);
  assert(high >= 0 && low >= 0, `invalid rank in class: ${text}`);
  assert(high >= low, `class ranks must be in descending order: ${text}`);
  const kind = text.length === 3 ? text[2] : "";
  assert(["", "s", "o"].includes(kind), `class suffix must be s or o: ${text}`);
  assert(high !== low || kind === "", `pairs cannot have a suitedness suffix: ${text}`);
  return { high, low, kind };
}

function classMatches(item, low, high) {
  const suited = low % 4 === high % 4;
  return (
    Math.floor(low / 4) === item.low &&
    Math.floor(high / 4) === item.high &&
    (item.kind === "" || (item.kind === "s") === suited)
  );
}

export function expandExpression(expression) {
  if (expression.includes("-")) {
    const [first, ...rest] = expression.split("-");
    assert.equal(rest.length, 1, `interval needs exactly one dash: ${expression}`);
    const start = parseClass(first);
    const end = parseClass(rest[0]);
    assert.equal(start.kind, end.kind, `interval suffixes must match: ${expression}`);
    const startPair = start.high === start.low;
    assert.equal(startPair, end.high === end.low, `interval mixes a pair and a nonpair`);
    const lo = Math.min(start.low, end.low);
    const hi = Math.max(start.low, end.low);
    if (startPair) {
      const first_ = Math.min(start.high, end.high);
      const last = Math.max(start.high, end.high);
      return Array.from({ length: last - first_ + 1 }, (_, i) => ({
        high: first_ + i,
        low: first_ + i,
        kind: "",
      }));
    }
    if (start.high === end.high) {
      return Array.from({ length: hi - lo + 1 }, (_, i) => ({
        high: start.high,
        low: lo + i,
        kind: start.kind,
      }));
    }
    assert.equal(
      start.high - start.low,
      end.high - end.low,
      `interval must keep its high rank or its rank gap constant: ${expression}`,
    );
    const gap = start.high - start.low;
    return Array.from({ length: hi - lo + 1 }, (_, i) => ({
      high: lo + i + gap,
      low: lo + i,
      kind: start.kind,
    }));
  }
  if (expression.endsWith("+")) {
    const base = parseClass(expression.slice(0, -1));
    if (base.high === base.low) {
      return Array.from({ length: 13 - base.high }, (_, i) => ({
        high: base.high + i,
        low: base.high + i,
        kind: "",
      }));
    }
    return Array.from({ length: base.high - base.low }, (_, i) => ({
      high: base.high,
      low: base.low + i,
      kind: base.kind,
    }));
  }
  if (expression.length === 4) {
    const a = cardId(expression.slice(0, 2));
    const b = cardId(expression.slice(2));
    assert(a !== b, `expected two distinct cards: ${expression}`);
    return { explicit: [Math.min(a, b), Math.max(a, b)] };
  }
  return [parseClass(expression)];
}

export function expandRange(text) {
  assert(typeof text === "string" && text.length <= 1024);
  const tokens = [];
  for (const part of text.split(",")) {
    assert(part.trim().length > 0, "empty comma item in range");
    for (const token of part.split(/\s+/).filter((t) => t.length > 0)) tokens.push(token);
  }
  assert(tokens.length > 0 && tokens.length <= 64, "unsupported range token count");
  const weights = new Float32Array(1326);
  const assigned = new Uint8Array(1326);
  const index = new Map();
  let cursor = 0;
  for (let low = 0; low < 52; low++) {
    for (let high = low + 1; high < 52; high++) index.set(low * 52 + high, cursor++);
  }
  assert.equal(cursor, 1326);
  for (const token of tokens) {
    const colon = token.indexOf(":");
    const expression = colon < 0 ? token : token.slice(0, colon);
    const weight = colon < 0 ? 1 : Number(token.slice(colon + 1));
    assert(Number.isFinite(weight) && weight >= 0 && weight <= 1, `invalid weight: ${token}`);
    const expanded = expandExpression(expression);
    const targets = [];
    if (Array.isArray(expanded)) {
      for (let low = 0; low < 52; low++) {
        for (let high = low + 1; high < 52; high++) {
          if (expanded.some((item) => classMatches(item, low, high))) targets.push(low * 52 + high);
        }
      }
    } else {
      targets.push(expanded.explicit[0] * 52 + expanded.explicit[1]);
    }
    assert(targets.length > 0, `token matched no combo: ${token}`);
    for (const target of targets) {
      const slot = index.get(target);
      assert(
        !assigned[slot] || weights[slot] === Math.fround(weight),
        `conflicting assignment for ${token}`,
      );
      assigned[slot] = 1;
      weights[slot] = weight;
    }
  }
  return weights;
}

export function rangeHands(text, board) {
  const dead = new Set(board.map(cardId));
  const weights = expandRange(text);
  const hands = [];
  let cursor = 0;
  for (let low = 0; low < 52; low++) {
    for (let high = low + 1; high < 52; high++) {
      const weight = weights[cursor++];
      if (weight > 0 && !dead.has(low) && !dead.has(high)) hands.push([low, high]);
    }
  }
  return hands;
}

export function decodePrivateCards(packed) {
  return Array.from(packed, (pair) => {
    const low = pair & 255;
    const high = pair >>> 8;
    assert(low < high && high < 52);
    return { cards: [cardLabel(low), cardLabel(high)], ids: [low, high] };
  });
}

export function decodeActions(text) {
  if (text === "terminal") return [];
  if (text === "chance") return null;
  return text.split("/").map((item) => {
    const match = /^(Fold|Check|Call|Bet|Raise|Allin):(\d+)$/.exec(item);
    assert(match, `Invalid reference action: ${item}`);
    const kind = match[1].toLowerCase();
    const amount = Number(match[2]);
    assert(Number.isSafeInteger(amount));
    if (["fold", "check", "call"].includes(kind)) {
      assert.equal(amount, 0);
      return { kind, label: kind, amount: null };
    }
    return { kind, label: `${kind}:${amount}`, amount };
  });
}

function finiteArray(values, label, nonnegative = false) {
  assert(
    values.every((v) => Number.isFinite(v) && (!nonnegative || v >= 0)),
    label,
  );
  return values;
}

// Same buffer layout as the river driver; `player === null` covers terminal and chance nodes,
// neither of which carries a strategy block.
export function decodeResults(buffer, counts, player, actionCount, pot, contributions) {
  let cursor = 0;
  const take = (count) => {
    const values = Array.from(buffer.slice(cursor, cursor + count));
    cursor += count;
    assert.equal(values.length, count, "Truncated reference result");
    return values;
  };
  const takeBoth = () => counts.map((count) => take(count));
  const [pot0, pot1, emptyFlag] = take(3);
  assert(Number.isInteger(emptyFlag) && emptyFlag >= 0 && emptyFlag <= 3);
  const matched = Math.min(...contributions);
  assert.equal(pot0, pot + matched + contributions[0]);
  assert.equal(pot1, pot + matched + contributions[1]);
  const reach = takeBoth();
  reach.forEach((row) => finiteArray(row, "Invalid reported reach", true));
  const normalized = takeBoth();
  normalized.forEach((row) => finiteArray(row, "Invalid reported normalized weight", true));
  let equity = counts.map((n) => Array(n).fill(null));
  let ev = counts.map((n) => Array(n).fill(null));
  if (emptyFlag === 0) {
    equity = takeBoth();
    ev = takeBoth();
    // EQR may contain infinities; it is deliberately neither serialized nor accepted.
    takeBoth();
  }
  const isDecision = player === 0 || player === 1;
  const strategy = isDecision ? take(actionCount * counts[player]) : [];
  const detail = isDecision && emptyFlag === 0 ? take(strategy.length) : [];
  assert.equal(cursor, buffer.length, "Unexpected reference result layout");
  const available = counts.map((n, p) =>
    Array.from({ length: n }, (_, h) => emptyFlag === 0 && normalized[p][h] > 0),
  );
  for (let p = 0; p < 2; p++) {
    for (let h = 0; h < counts[p]; h++) {
      if (!available[p][h]) {
        equity[p][h] = null;
        ev[p][h] = null;
      } else {
        assert(Number.isFinite(ev[p][h]));
        assert(Number.isFinite(equity[p][h]) && equity[p][h] >= 0 && equity[p][h] <= 1);
      }
    }
  }
  const actionEv = isDecision ? Array(strategy.length).fill(null) : [];
  if (isDecision) {
    for (let h = 0; h < counts[player]; h++) {
      let sum = 0;
      for (let a = 0; a < actionCount; a++) {
        const index = a * counts[player] + h;
        assert(Number.isFinite(strategy[index]) && strategy[index] >= 0 && strategy[index] <= 1);
        sum += strategy[index];
        if (available[player][h]) {
          assert(Number.isFinite(detail[index]));
          actionEv[index] = detail[index];
        }
      }
      assert(Math.abs(sum - 1) <= 0.00002, "Reference strategy does not sum to one");
    }
  }
  return {
    wasm_empty_range_flag: emptyFlag,
    reach_weights: reach,
    normalized_weights: emptyFlag === 0 ? normalized : [null, null],
    ev_available: available,
    equity,
    expected_values: ev,
    strategy,
    action_expected_values: actionEv,
  };
}

function weightedMean(values, weights) {
  let numerator = 0;
  let denominator = 0;
  values.forEach((value, index) => {
    if (value !== null && weights[index] > 0) {
      numerator += value * weights[index];
      denominator += weights[index];
    }
  });
  assert(denominator > 0, "Root has no compatible reference mass");
  return numerator / denominator;
}

export function possibleCardLabels(mask) {
  const labels = [];
  for (let card = 0; card < 52; card++) {
    if ((mask & (1n << BigInt(card))) !== 0n) labels.push(cardLabel(card));
  }
  return labels;
}

export function initArguments(input) {
  const menus = input.menus;
  return [
    expandRange(input.ranges[0]),
    expandRange(input.ranges[1]),
    null, // board, filled by the caller
    input.starting_pot,
    input.effective_stack,
    0, // rake_rate
    0, // rake_cap
    input.donk_option,
    menus.flop.oop_bet,
    menus.flop.oop_raise,
    menus.turn.oop_bet,
    menus.turn.oop_raise,
    menus.turn.oop_donk,
    menus.river.oop_bet,
    menus.river.oop_raise,
    menus.river.oop_donk,
    menus.flop.ip_bet,
    menus.flop.ip_raise,
    menus.turn.ip_bet,
    menus.turn.ip_raise,
    menus.river.ip_bet,
    menus.river.ip_raise,
    input.add_all_in_threshold,
    input.force_all_in_threshold,
    0, // merging_threshold
    "", // added_lines
    "", // removed_lines
  ];
}

export function captureCase(GameManager, input, finishBudget = false) {
  const started = performance.now();
  const game = GameManager.new();
  try {
    const board = Uint8Array.from(input.board.map(cardId));
    // Sorting the flop matches upstream's board input convention; the turn card stays last.
    board.subarray(0, 3).sort();
    const args = initArguments(input);
    args[2] = board;
    const error = game.init(...args);
    assert(error == null, `Reference initialization failed: ${error}`);
    const memory = Number(game.memory_usage(false));
    assert(
      Number.isSafeInteger(memory) && memory > 0 && memory <= 2 * 1024 * 1024 * 1024,
      "Reference memory estimate exceeds 2 GiB",
    );
    game.allocate_memory(false);
    const privateCards = [0, 1].map((p) => decodePrivateCards(game.private_cards(p)));
    const counts = privateCards.map((cards) => cards.length);
    assert(counts.every((n) => n > 0 && n <= 1326));
    let exploitability = game.exploitability();
    const checkpoints = [];
    const record = (iterations) => {
      assert(Number.isFinite(exploitability), "Nonfinite reference exploitability");
      checkpoints.push({
        iterations,
        exploitability_chips: exploitability,
        elapsed_seconds: (performance.now() - started) / 1000,
      });
    };
    record(0);
    const target = (input.starting_pot * input.target_pct_of_pot) / 100;
    let iterations = 0;
    while ((finishBudget || exploitability > target) && iterations < input.max_iterations) {
      game.solve_step(iterations);
      iterations++;
      if (iterations % input.check_every === 0 || iterations === input.max_iterations) {
        exploitability = game.exploitability();
        record(iterations);
      }
    }
    game.finalize();
    const exportRunouts = input.export_runouts.map(cardId);
    const nodes = [];
    let rootResults = null;
    const pending = [{ indices: [], labels: [], street: "turn", runout: null, parentPlayer: null }];
    while (pending.length) {
      assert(nodes.length < MAX_NODES, `Reference exceeds ${MAX_NODES} exported nodes`);
      const history = pending.pop();
      assert(history.indices.length <= 64, "Reference history exceeds 64 actions");
      game.apply_history(Uint32Array.from(history.indices));
      const role = game.current_player();
      assert(["oop", "ip", "terminal", "chance"].includes(role));
      const player = role === "oop" ? 0 : role === "ip" ? 1 : null;
      const actions = decodeActions(game.actions_after(new Uint32Array()));
      const reported = Array.from(game.total_bet_amount(new Uint32Array()));
      assert.equal(reported.length, 2);
      assert(reported.every((n) => Number.isInteger(n) && n >= 0 && n <= input.effective_stack));
      const last = history.labels.at(-1);
      const isFold = role === "terminal" && last === "fold";
      const contributions = isFold ? [Math.min(...reported), Math.min(...reported)] : reported;
      const results = decodeResults(
        game.get_results(),
        counts,
        player,
        actions === null ? 0 : actions.length,
        input.starting_pot,
        reported,
      );
      if (nodes.length === 0) rootResults = results;
      const common = {
        history: history.indices,
        history_labels: history.labels,
        street: history.street,
        runout: history.runout,
        contributions,
        reported_contributions: reported,
        wasm_empty_range_flag: results.wasm_empty_range_flag,
      };
      if (role === "chance") {
        assert.equal(actions, null);
        assert.equal(history.street, "turn", "Only the turn tree's single chance layer is expected");
        const mask = game.possible_cards();
        assert(typeof mask === "bigint", "Expected a 64-bit possible-card mask");
        const possible = possibleCardLabels(mask);
        const representatives = game.num_actions();
        assert(
          representatives > 0 && representatives <= possible.length,
          "Chance node representative count is out of range",
        );
        nodes.push({
          ...common,
          kind: "chance",
          player: null,
          actions: [],
          terminal: null,
          fold_winner: null,
          possible_cards: possible,
          representative_action_count: representatives,
          isomorphic_merged_cards: possible.length - representatives,
          exported_runouts: input.export_runouts,
          reach_weights: results.reach_weights,
          normalized_weights: results.normalized_weights,
          expected_values: results.expected_values,
          ev_available: results.ev_available,
        });
        for (let index = exportRunouts.length - 1; index >= 0; index--) {
          const card = exportRunouts[index];
          assert(
            possible.includes(cardLabel(card)),
            `Exported runout ${cardLabel(card)} is not dealable here`,
          );
          pending.push({
            indices: [...history.indices, card],
            labels: [...history.labels, `chance:${cardLabel(card)}`],
            street: "river",
            runout: cardLabel(card),
            parentPlayer: null,
          });
        }
        continue;
      }
      if (role === "terminal") {
        assert(Array.isArray(actions) && actions.length === 0);
        // River showdowns are recomputed exactly by oracle.py from the five-card board, so
        // they need no reported values. A turn-street showdown is an all-in called before the
        // river: its value spans every runout, so the reported per-hand value is kept as the
        // review's continuation value.
        const reported_values =
          history.street === "turn" && !isFold
            ? {
                reach_weights: results.reach_weights,
                normalized_weights: results.normalized_weights,
                expected_values: results.expected_values,
                ev_available: results.ev_available,
              }
            : {};
        nodes.push({
          ...common,
          kind: "terminal",
          player: null,
          actions: [],
          terminal: isFold ? "fold" : "showdown",
          fold_winner: isFold ? 1 - history.parentPlayer : null,
          ...reported_values,
        });
        continue;
      }
      assert(Array.isArray(actions) && actions.length > 0);
      assert.equal(actions.length, game.num_actions());
      nodes.push({
        ...common,
        kind: "decision",
        player,
        actions,
        terminal: null,
        fold_winner: null,
        reach_weights: results.reach_weights[player],
        ev_available: results.ev_available[player],
        strategy: results.strategy,
        action_expected_values: results.action_expected_values,
      });
      for (let index = actions.length - 1; index >= 0; index--) {
        pending.push({
          indices: [...history.indices, index],
          labels: [...history.labels, actions[index].label],
          street: history.street,
          runout: history.runout,
          parentPlayer: player,
        });
      }
    }
    assert(rootResults !== null, "Missing root results");
    const rootEv = [0, 1].map((p) =>
      weightedMean(rootResults.expected_values[p], rootResults.normalized_weights[p]),
    );
    return {
      input,
      private_cards: privateCards,
      nodes,
      iterations,
      execution_stop_policy: finishBudget ? "fixed_iteration_budget" : "target_or_cap",
      stop_reason: finishBudget
        ? "fixed_iteration_budget"
        : exploitability <= target
          ? "target"
          : "iteration_cap",
      exploitability_chips: exploitability,
      exploitability_pct_of_pot: (100 * exploitability) / input.starting_pot,
      root_expected_values: rootEv,
      root_centered_expected_values: rootEv.map((ev) => ev - input.starting_pot / 2),
      root_normalized_weights: rootResults.normalized_weights,
      checkpoints,
      reference_memory_estimate_bytes: memory,
      wasm_memory_bytes: null,
      elapsed_seconds: (performance.now() - started) / 1000,
    };
  } finally {
    game.free();
  }
}

export function parseFlags(args) {
  const flags = args.slice(3);
  assert(
    args.length >= 3 &&
      flags.length <= 2 &&
      new Set(flags).size === flags.length &&
      flags.every((flag) => ["--finish-budget", "--raw-display"].includes(flag)),
    "Expected three file arguments and optional --finish-budget / --raw-display",
  );
  return {
    finishBudget: flags.includes("--finish-budget"),
    rawDisplay: flags.includes("--raw-display"),
  };
}

export function presentationMetadata(rawDisplay) {
  return {
    reach_display_cutoff: rawDisplay ? 0 : 0.0005,
    values_rounded_by_upstream: !rawDisplay,
    values_below_1_decimal_places: rawDisplay ? null : 6,
    arithmetic_precision: "f32",
    zero_reach_evs: "null",
  };
}

export function main(args) {
  const { finishBudget, rawDisplay } = parseFlags(args);
  assert.equal(process.version, "v24.19.0", "Use the pinned Node version");
  const require = createRequire(import.meta.url);
  const bindings = require(args[0]);
  const payload = JSON.parse(readFileSync(args[1], "utf8"));
  assert.equal(payload.street, "turn", "This driver captures turn-start trees only");
  const result = {
    schema_version: 1,
    capture_version: CAPTURE_VERSION,
    street: "turn",
    execution_stop_policy: finishBudget ? "fixed_iteration_budget" : "target_or_cap",
    presentation_mode: rawDisplay ? "raw_f32" : "upstream_display",
    runtime: {
      node: process.version,
      v8: process.versions.v8,
      platform: process.platform,
      architecture: process.arch,
    },
    interface: {
      strategy_layout: "action_major",
      ...presentationMetadata(rawDisplay),
      eqr_exported: false,
      equity_exported: false,
      ev_origin: "current_decision_fold_zero",
      root_centered_ev_conversion: "display_ev - starting_pot / 2 - reported_contributions[player]",
      terminal_contributions: "after_refund",
      compression: false,
      rake_rate: 0,
      rake_cap: 0,
      merging_threshold: 0,
      bunching: false,
      chance_history_encoding: "card_id",
      chance_isomorphism: "upstream merges isomorphic runouts; playing a merged card replays its representative and swaps the suits back, so exported rows are indexed by the dealt card",
      exported_runout_scope: "case.export_runouts only; the solve and the exploitability cover every runout",
      decision_node_rows: "acting player only",
      reference_raise_cap: null,
      reference_raise_cap_note: "Input cap 32 must be checked against every exported history",
    },
    cases: payload.cases.map((input) => captureCase(bindings.GameManager, input, finishBudget)),
  };
  result.runtime.peak_rss_bytes = process.resourceUsage().maxRSS * 1024;
  result.runtime.memory_at_end = process.memoryUsage();
  const serialized = JSON.stringify(result, (_, value) => {
    assert(typeof value !== "number" || Number.isFinite(value), "Nonfinite JSON output");
    return value;
  });
  assert(Buffer.byteLength(serialized) <= MAX_OUTPUT_BYTES, "Reference output exceeds 64 MiB");
  writeFileSync(args[2], serialized + "\n", { flag: "wx" });
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2));
}
