// Independent reference-process driver. Imports no application module.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { performance } from "node:perf_hooks";
import { pathToFileURL } from "node:url";

export const CAPTURE_VERSION = 1;
export const MAX_OUTPUT_BYTES = 64 * 1024 * 1024;
const RANKS = "23456789TJQKA";
const SUITS = "cdhs";

export function cardId(label) {
  assert.match(label, /^[2-9TJQKA][cdhs]$/);
  return 4 * RANKS.indexOf(label[0]) + SUITS.indexOf(label[1]);
}

function cardLabel(id) {
  assert(Number.isInteger(id) && id >= 0 && id < 52);
  return RANKS[Math.floor(id / 4)] + SUITS[id % 4];
}

// Only the explicitly supported test-input vocabulary, not a Pio parser.
export function expandRange(text) {
  assert(typeof text === "string" && text.length <= 1024);
  const classes = new Set(text.split(","));
  assert(classes.size > 0 && classes.size <= 32);
  for (const item of classes) {
    assert.match(item, /^[2-9TJQKA]{2}$/);
    assert(RANKS.indexOf(item[0]) >= RANKS.indexOf(item[1]));
  }
  const weights = [];
  for (let low = 0; low < 52; low++) {
    for (let high = low + 1; high < 52; high++) {
      const group = RANKS[Math.floor(high / 4)] + RANKS[Math.floor(low / 4)];
      weights.push(classes.has(group) ? 1 : 0);
    }
  }
  assert.equal(weights.length, 1326);
  return new Float32Array(weights);
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
  assert(text !== "chance", "A river-only reference cannot have chance nodes");
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
  assert(values.every((v) => Number.isFinite(v) && (!nonnegative || v >= 0)), label);
  return values;
}

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

export function captureCase(GameManager, input) {
  const started = performance.now();
  const game = GameManager.new();
  try {
    const board = Uint8Array.from(input.board.map(cardId));
    // Sorting the flop matches upstream's board input convention.
    board.subarray(0, 3).sort();
    const error = game.init(
      expandRange(input.ranges[0]), expandRange(input.ranges[1]), board,
      input.starting_pot, input.effective_stack, 0, 0, false,
      "", "", "", "", "", input.bets[0], input.raises[0], "",
      "", "", "", "", input.bets[1], input.raises[1], 0, 0, 0, "", "",
    );
    assert(error == null, `Reference initialization failed: ${error}`);
    const memory = Number(game.memory_usage(false));
    assert(Number.isSafeInteger(memory) && memory > 0 && memory <= 512 * 1024 * 1024,
      "Reference memory estimate exceeds 512 MiB");
    game.allocate_memory(false);
    const privateCards = [0, 1].map((p) => decodePrivateCards(game.private_cards(p)));
    const counts = privateCards.map((cards) => cards.length);
    assert(counts.every((n) => n > 0 && n <= 1326));
    let exploitability = game.exploitability();
    const checkpoints = [];
    const record = (iterations) => {
      assert(Number.isFinite(exploitability), "Nonfinite reference exploitability");
      checkpoints.push({ iterations, exploitability_chips: exploitability,
        elapsed_seconds: (performance.now() - started) / 1000 });
    };
    record(0);
    const target = input.starting_pot * input.target_pct_of_pot / 100;
    let iterations = 0;
    while (exploitability > target && iterations < input.max_iterations) {
      game.solve_step(iterations);
      iterations++;
      if (iterations % input.check_every === 0 || iterations === input.max_iterations) {
        exploitability = game.exploitability();
        record(iterations);
      }
    }
    game.finalize();
    const nodes = [];
    const pending = [{ indices: [], labels: [] }];
    while (pending.length) {
      assert(nodes.length < 10000, "Reference exceeds 10,000 public nodes");
      const history = pending.pop();
      assert(history.indices.length <= 64, "Reference history exceeds 64 actions");
      game.apply_history(Uint32Array.from(history.indices));
      const role = game.current_player();
      assert(["oop", "ip", "terminal"].includes(role));
      const player = role === "oop" ? 0 : role === "ip" ? 1 : null;
      const actions = decodeActions(game.actions_after(new Uint32Array()));
      assert.equal(actions.length, game.num_actions());
      const reported = Array.from(game.total_bet_amount(new Uint32Array()));
      assert.equal(reported.length, 2);
      assert(reported.every((n) => Number.isInteger(n) && n >= 0 && n <= input.effective_stack));
      const last = history.labels.at(-1);
      const isFold = role === "terminal" && last === "fold";
      const contributions = isFold ? [Math.min(...reported), Math.min(...reported)] : reported;
      const results = decodeResults(game.get_results(), counts, player, actions.length,
        input.starting_pot, reported);
      nodes.push({ history: history.indices, history_labels: history.labels,
        kind: role === "terminal" ? "terminal" : "decision", player, actions,
        terminal: role !== "terminal" ? null : isFold ? "fold" : "showdown",
        // OOP acts at even depth; the last actor folded.
        fold_winner: isFold ? 1 - ((history.indices.length - 1) % 2) : null,
        contributions, reported_contributions: reported, ...results });
      for (let index = actions.length - 1; index >= 0; index--) {
        pending.push({ indices: [...history.indices, index],
          labels: [...history.labels, actions[index].label] });
      }
    }
    const root = nodes[0];
    const rootEv = [0, 1].map((p) => weightedMean(root.expected_values[p], root.normalized_weights[p]));
    return { input, private_cards: privateCards, nodes, iterations,
      stop_reason: exploitability <= target ? "target" : "iteration_cap",
      exploitability_chips: exploitability,
      exploitability_pct_of_pot: 100 * exploitability / input.starting_pot,
      root_expected_values: rootEv,
      root_centered_expected_values: rootEv.map((ev) => ev - input.starting_pot / 2),
      checkpoints, reference_memory_estimate_bytes: memory,
      wasm_memory_bytes: null,
      elapsed_seconds: (performance.now() - started) / 1000 };
  } finally {
    game.free();
  }
}

export function main(args) {
  assert.equal(args.length, 3, "Expected WASM bindings, validated inputs and output path");
  assert.equal(process.version, "v24.19.0", "Use the pinned Node version");
  const require = createRequire(import.meta.url);
  const bindings = require(args[0]);
  const payload = JSON.parse(readFileSync(args[1], "utf8"));
  const result = {
    schema_version: 1, capture_version: CAPTURE_VERSION,
    runtime: { node: process.version, v8: process.versions.v8,
      platform: process.platform, architecture: process.arch },
    interface: { strategy_layout: "action_major",
      reach_display_cutoff: 0.0005, values_rounded_by_upstream: true,
      values_below_1_decimal_places: 6,
      zero_reach_evs: "null", eqr_exported: false,
      ev_origin: "current_decision_fold_zero",
      root_centered_ev_conversion: "display_ev - starting_pot / 2 - reported_contributions[player]",
      terminal_contributions: "after_refund", compression: false, rake_rate: 0,
      rake_cap: 0, merging_threshold: 0, bunching: false,
      reference_raise_cap: null,
      reference_raise_cap_note: "Input cap 32 must be checked against every exported history" },
    cases: payload.cases.map((input) => captureCase(bindings.GameManager, input)),
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
