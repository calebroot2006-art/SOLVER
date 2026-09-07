// Guards on the turn driver's pure functions. Runs with `node --test`; no WASM build needed.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  cardId,
  cardLabel,
  captureCase,
  decodeActions,
  decodeResults,
  expandExpression,
  expandRange,
  initArguments,
  parseFlags,
  possibleCardLabels,
  rangeHands,
} from "./capture.mjs";

const CASES = JSON.parse(readFileSync(new URL("./cases.json", import.meta.url), "utf8"));

test("card identifiers round-trip", () => {
  for (let id = 0; id < 52; id++) assert.equal(cardId(cardLabel(id)), id);
  assert.equal(cardId("2c"), 0);
  assert.equal(cardId("As"), 51);
  assert.throws(() => cardId("1x"));
});

test("range intervals follow the project parser", () => {
  assert.equal(expandRange("22-44").length, 1326);
  assert.equal([...expandRange("22-44")].filter((w) => w > 0).length, 18);
  assert.equal([...expandRange("A2s-A5s")].filter((w) => w > 0).length, 16);
  assert.equal([...expandRange("98s-65s")].filter((w) => w > 0).length, 16);
  assert.equal([...expandRange("22+")].filter((w) => w > 0).length, 78);
  assert.equal([...expandRange("ATo+")].filter((w) => w > 0).length, 48);
  assert.deepEqual(expandExpression("AhKd"), { explicit: [cardId("Kd"), cardId("Ah")] });
});

test("weights survive and conflicts are refused", () => {
  const weights = expandRange("JJ:0.5,TT");
  assert.equal([...weights].filter((w) => w === 0.5).length, 6);
  assert.equal([...weights].filter((w) => w === 1).length, 6);
  assert.throws(() => expandRange("AA:0.5,AA:0.25"));
  assert.throws(() => expandRange("AA,,KK"));
});

test("the committed cases expand to live turn ranges", () => {
  const expected = {
    turn_100bb_dry_rainbow: [469, 470],
    turn_100bb_paired: [468, 473],
    turn_100bb_flush_possible: [445, 448],
  };
  for (const input of CASES.cases) {
    const live = [0, 1].map((p) => rangeHands(input.ranges[p], input.board).length);
    assert.deepEqual(live, expected[input.id], input.id);
  }
});

test("chance nodes decode to null, terminals to an empty list", () => {
  assert.equal(decodeActions("chance"), null);
  assert.deepEqual(decodeActions("terminal"), []);
  assert.deepEqual(decodeActions("Check:0/Bet:3"), [
    { kind: "check", label: "check", amount: null },
    { kind: "bet", label: "bet:3", amount: 3 },
  ]);
  assert.throws(() => decodeActions("Wager:3"));
});

test("the init argument list matches the pinned binding signature", () => {
  const input = CASES.cases[0];
  const args = initArguments(input);
  // 27 arguments: two ranges, the board, five scalars, sixteen size strings, three
  // thresholds and two line strings. Positions 2 is the board, filled by the caller.
  assert.equal(args.length, 27);
  assert.equal(args[2], null);
  assert.equal(args[3], input.starting_pot);
  assert.equal(args[4], input.effective_stack);
  assert.equal(args[5], 0, "rake rate");
  assert.equal(args[6], 0, "rake cap");
  assert.equal(args[7], false, "donk option");
  assert.deepEqual(args.slice(8, 16), [
    "", // oop_flop_bet
    "", // oop_flop_raise
    input.menus.turn.oop_bet,
    input.menus.turn.oop_raise,
    input.menus.turn.oop_donk,
    input.menus.river.oop_bet,
    input.menus.river.oop_raise,
    input.menus.river.oop_donk,
  ]);
  assert.deepEqual(args.slice(16, 22), [
    "", // ip_flop_bet
    "", // ip_flop_raise
    input.menus.turn.ip_bet,
    input.menus.turn.ip_raise,
    input.menus.river.ip_bet,
    input.menus.river.ip_raise,
  ]);
  assert.deepEqual(args.slice(22), [0, 0, 0, "", ""]);
});

test("possible-card masks decode to labels", () => {
  const mask = (1n << BigInt(cardId("2c"))) | (1n << BigInt(cardId("As")));
  assert.deepEqual(possibleCardLabels(mask), ["2c", "As"]);
  assert.deepEqual(possibleCardLabels(0n), []);
});

function buffer(counts, player, actionCount) {
  const values = [10, 10, 0];
  const both = (fill) => {
    for (let p = 0; p < 2; p++) for (let h = 0; h < counts[p]; h++) values.push(fill(p, h));
  };
  both(() => 1); // reach
  both(() => 1); // normalized
  both(() => 0.5); // equity
  both(() => 5); // expected values
  both(() => 1); // eqr, read and discarded
  if (player !== null) {
    for (let a = 0; a < actionCount; a++) {
      for (let h = 0; h < counts[player]; h++) values.push(a === 0 ? 0.25 : 0.75);
    }
    for (let a = 0; a < actionCount; a++) {
      for (let h = 0; h < counts[player]; h++) values.push(a);
    }
  }
  return Float64Array.from(values);
}

test("a decision node decodes its strategy block", () => {
  const counts = [2, 3];
  const decoded = decodeResults(buffer(counts, 0, 2), counts, 0, 2, 10, [0, 0]);
  assert.equal(decoded.strategy.length, 4);
  assert.deepEqual(decoded.strategy, [0.25, 0.25, 0.75, 0.75]);
  assert.deepEqual(decoded.action_expected_values, [0, 0, 1, 1]);
  assert.deepEqual(decoded.ev_available, [
    [true, true],
    [true, true, true],
  ]);
});

test("a chance node has no strategy block", () => {
  const counts = [2, 3];
  const decoded = decodeResults(buffer(counts, null, 0), counts, null, 0, 10, [0, 0]);
  assert.deepEqual(decoded.strategy, []);
  assert.deepEqual(decoded.action_expected_values, []);
  assert.deepEqual(decoded.expected_values, [
    [5, 5],
    [5, 5, 5],
  ]);
});

test("a truncated result buffer is refused", () => {
  const counts = [2, 3];
  const values = buffer(counts, 0, 2).slice(0, -1);
  assert.throws(() => decodeResults(values, counts, 0, 2, 10, [0, 0]));
});

test("only the two documented flags are accepted", () => {
  assert.deepEqual(parseFlags(["a", "b", "c"]), { finishBudget: false, rawDisplay: false });
  assert.deepEqual(parseFlags(["a", "b", "c", "--finish-budget"]), {
    finishBudget: true,
    rawDisplay: false,
  });
  assert.throws(() => parseFlags(["a", "b", "c", "--threads"]));
  assert.throws(() => parseFlags(["a", "b", "c", "--raw-display", "--raw-display"]));
});

test("captureCase refuses a binding that fails to initialize", () => {
  const stub = {
    new: () => ({
      init: () => "Invalid board length",
      free: () => {},
    }),
  };
  assert.throws(() => captureCase(stub, CASES.cases[0]), /Reference initialization failed/);
});
