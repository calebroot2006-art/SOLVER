import assert from "node:assert/strict";
import test from "node:test";
import { cardId, decodeActions, decodePrivateCards, decodeResults, expandRange } from "./capture.mjs";

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
