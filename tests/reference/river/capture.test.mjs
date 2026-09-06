import assert from "node:assert/strict";
import test from "node:test";
import { captureCase, cardId, decodeActions, decodePrivateCards, decodeResults, expandRange,
  parseFlags, presentationMetadata } from "./capture.mjs";

test("presentation flags compose without accepting ambiguous arguments", () => {
  const paths = ["binding.js", "input.json", "output.json"];
  assert.deepEqual(parseFlags(paths), { finishBudget: false, rawDisplay: false });
  assert.deepEqual(parseFlags([...paths, "--raw-display"]), { finishBudget: false, rawDisplay: true });
  for (const flags of [["--finish-budget", "--raw-display"], ["--raw-display", "--finish-budget"]]) {
    assert.deepEqual(parseFlags([...paths, ...flags]), { finishBudget: true, rawDisplay: true });
  }
  for (const flags of [["--unknown"], ["--raw-display", "--raw-display"],
    ["--finish-budget", "--finish-budget"], ["--raw-display", "--finish-budget", "extra"]]) {
    assert.throws(() => parseFlags([...paths, ...flags]));
  }
  assert.throws(() => parseFlags(paths.slice(0, 2)));
  assert.deepEqual(presentationMetadata(false), { reach_display_cutoff: 0.0005,
    values_rounded_by_upstream: true, values_below_1_decimal_places: 6,
    arithmetic_precision: "f32", zero_reach_evs: "null" });
  assert.deepEqual(presentationMetadata(true), { reach_display_cutoff: 0,
    values_rounded_by_upstream: false, values_below_1_decimal_places: null,
    arithmetic_precision: "f32", zero_reach_evs: "null" });
});

test("physical card mapping and range expansion preserve all suits", () => {
  assert.equal(cardId("2c"), 0);
  assert.equal(cardId("As"), 51);
  assert.equal(expandRange("AA").reduce((a, b) => a + b), 6);
  assert.equal(expandRange("AK").reduce((a, b) => a + b), 16);
  assert.equal(expandRange("AA,AK").reduce((a, b) => a + b), 22);
  const combo = decodePrivateCards([0 | (51 << 8)])[0];
  assert.deepEqual(combo.cards, ["2c", "As"]);
  assert.equal(expandRange("A2")[50], 1); // First low card, high ID 51.
  assert.throws(() => expandRange("KA"));
  assert.throws(() => expandRange("AA:0.5"));
  assert.throws(() => cardId("10s"));
});

test("action labels preserve cumulative targets and reject chance nodes", () => {
  assert.deepEqual(decodeActions("Fold:0/Call:0/Allin:20").map((a) => a.label),
    ["fold", "call", "allin:20"]);
  assert.deepEqual(decodeActions("terminal"), []);
  assert.throws(() => decodeActions("chance"));
  assert.throws(() => decodeActions("Check:1"));
  assert.throws(() => decodeActions("Raise:NaN"));
});

// Synthetic protocol buffers only. None is a solver output or reference fixture.
const completeBuffer = () => [
  10, 10, 0, // pot headers and no empty range
  1, 0, 1, // reported reach: OOP 2 hands; IP 1
  1, 0, 1, // normalized weights
  1, 0, 0, // equity
  10, 0, 0, // decision-origin EV
  1, Infinity, Infinity, // EQR, deliberately discarded
  0.25, 0.5, 0.75, 0.5, // action-major policy
  10, 0, 10, 0, // action-major EV
];

test("result decoder retains every policy and marks zero-mass EV unavailable", () => {
  const result = decodeResults(completeBuffer(), [2, 1], 0, 2, 10, [0, 0]);
  assert.deepEqual(result.expected_values, [[10, null], [0]]);
  assert.deepEqual(result.action_expected_values, [10, null, 10, null]);
  assert.deepEqual(result.strategy, [0.25, 0.5, 0.75, 0.5]);
  assert(!JSON.stringify(result).includes("Infinity"));
});

test("raw tiny positive mass exposes EV without making actual zero mass available", () => {
  const buffer = completeBuffer();
  buffer[3] = Math.fround(0.0000002);
  buffer[4] = 1; // Positive own reach still has zero compatible normalized mass.
  buffer[6] = Math.fround(0.0000003);
  buffer[12] = Math.fround(1.23456789);
  buffer[18] = Math.fround(0.23456789);
  buffer[20] = Math.fround(1 - buffer[18]);
  buffer[22] = Math.fround(2.34567891);
  const result = decodeResults(buffer, [2, 1], 0, 2, 10, [0, 0]);
  assert.equal(result.reach_weights[0][0], buffer[3]);
  assert.equal(result.expected_values[0][0], buffer[12]);
  assert.equal(result.strategy[0], buffer[18]);
  assert.equal(result.action_expected_values[0], buffer[22]);
  assert.deepEqual(result.ev_available, [[true, false], [true]]);
  assert.equal(result.expected_values[0][1], null);
  assert.equal(result.action_expected_values[1], null);
});

test("unreachable history has its own short buffer and unavailable EVs", () => {
  const buffer = [10, 10, 1, 0, 0, 1, 0, 0, 1, 0.25, 0.5, 0.75, 0.5];
  const result = decodeResults(buffer, [2, 1], 0, 2, 10, [0, 0]);
  assert.deepEqual(result.normalized_weights, [null, null]);
  assert.deepEqual(result.action_expected_values, [null, null, null, null]);
  assert.equal(result.strategy.length, 4);
});

test("terminal decoder checks contributions and rejects missing or added data", () => {
  const terminal = completeBuffer().slice(0, 18);
  assert.equal(decodeResults(terminal, [2, 1], null, 0, 10, [0, 0]).strategy.length, 0);
  assert.throws(() => decodeResults(completeBuffer().slice(1), [2, 1], 0, 2, 10, [0, 0]));
  assert.throws(() => decodeResults([...completeBuffer(), 1], [2, 1], 0, 2, 10, [0, 0]));
  assert.throws(() => decodeResults(completeBuffer(), [2, 1], 0, 2, 10, [5, 0]));
});

test("decoder rejects nonfinite reachable EV and denormalized policy", () => {
  const badEv = completeBuffer();
  badEv[12] = NaN;
  assert.throws(() => decodeResults(badEv, [2, 1], 0, 2, 10, [0, 0]));
  const badPolicy = completeBuffer();
  badPolicy[18] = 0.9;
  assert.throws(() => decodeResults(badPolicy, [2, 1], 0, 2, 10, [0, 0]));
});

test("fixed budget completes all updates even when the initial residual reaches target", () => {
  // A synthetic protocol stub tests control flow; it performs no poker calculation.
  const makeGame = () => ({
    steps: [], init: () => null, memory_usage: () => 1, allocate_memory() {},
    private_cards: () => [0 | (1 << 8)], exploitability: () => 0,
    solve_step(i) { this.steps.push(i); }, finalize() {}, apply_history() {},
    current_player: () => "terminal", actions_after: () => "terminal", num_actions: () => 0,
    total_bet_amount: () => [0, 0],
    get_results: () => [10, 10, 0, 1, 1, 1, 1, 0.5, 0.5, 5, 5, 1, 1], free() {},
  });
  const input = { board: ["Ac", "Kd", "7s", "4h", "2c"], ranges: ["AA", "QQ"],
    starting_pot: 10, effective_stack: 20, bets: ["", ""], raises: ["", ""],
    target_pct_of_pot: 0.001, max_iterations: 7, check_every: 3 };
  const earlyGame = makeGame();
  const early = captureCase({ new: () => earlyGame }, input);
  assert.deepEqual(earlyGame.steps, []);
  assert.equal(early.stop_reason, "target");
  assert.equal(early.execution_stop_policy, "target_or_cap");
  const fixedGame = makeGame();
  const fixed = captureCase({ new: () => fixedGame }, input, true);
  assert.deepEqual(fixedGame.steps, [0, 1, 2, 3, 4, 5, 6]);
  assert.equal(fixed.iterations, 7);
  assert.equal(fixed.stop_reason, "fixed_iteration_budget");
  assert.equal(fixed.execution_stop_policy, "fixed_iteration_budget");
  assert.deepEqual(fixed.checkpoints.map((p) => p.iterations), [0, 3, 6, 7]);
});
