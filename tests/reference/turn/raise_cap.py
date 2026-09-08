"""Derive the `removed_lines` string that prunes the pinned reference to our raise cap.

The pinned binding takes no raise cap (`GameManager::init` in wasm-postflop
`rust/solver-src/lib.rs:87-192` has bet-size strings and two line strings, and nothing else),
so a tree built from a 100%-pot raise menu keeps re-raising until the stack runs out. Decision
11 prunes it instead: upstream's `removed_lines` argument deletes exactly the branches our
one-raise tree does not have.

What the two upstream files say, at the pinned revisions:

* `lib.rs:179-188` splits `removed_lines` on `,` into lines, splits each line on `-` or `|`
  into tokens, and decodes a token with `decode_action` (`lib.rs:27-44`): `F` fold, `X` check,
  `C` call, `B<n>` bet, `R<n>` raise, `A<n>` all-in. `<n>` is the action's amount as the tree
  stores it, parsed as an `i32`. `-` and `|` are interchangeable to the parser; upstream's own
  UI writes `|` at a street change, and this module writes `-` throughout.
* postflop-solver `src/action_tree.rs:252-266` (`remove_line`) requires the line to exist in
  the current tree, and `remove_line_recursive` (`action_tree.rs:914-948`) removes the named
  action *and its child subtree* from its parent. Chance actions must be omitted from a line
  (`action_tree.rs:255`); the recursion walks through a chance node on its own
  (`action_tree.rs:923-925`), so a street change is implicit in the token sequence.
* A line that fails to resolve makes `init` return "Failed to remove line", which
  `capture.mjs` turns into a failed capture. So a derived line that does not exist is loud.
* `add_all_in_threshold` / `force_all_in_threshold` are the other two knobs that change which
  actions exist, and both act *before* removal: they are read inside `push_actions`
  (`action_tree.rs:657-691`) when the tree is built by `ActionTree::new`, which
  `lib.rs:165` calls before either line argument is applied. A raise whose clamped amount
  reaches the stack is already an `AllIn` action by then, so it is removed with an `A` token,
  not an `R` token. Both thresholds are 0 in every committed case.

Everything here is a pure function of the numbers in a case: menus, starting pot, effective
stack, thresholds and the cap. It replays the amount arithmetic of `push_actions`
(`action_tree.rs:526-737`) and `BuildTreeInfo::create_next` (`action_tree.rs:1003-1039`)
exactly, so it is testable on Windows without a WASM build. It imports nothing from the
project and no upstream source is copied into the repository.

The cap counts raises after the opening bet on one street, so a street may hold `cap + 1`
wagers in total. Every action that would be wager number `cap + 2` is removed; the walk does
not descend into a removed branch, so no derived line is a prefix of another and the order the
lines are applied in does not matter.
"""

from __future__ import annotations

import math
import re
from dataclasses import dataclass, replace

# Node player encoding, from action_tree.rs:8-14.
PLAYER_OOP = 0
PLAYER_IP = 1
PLAYER_CHANCE = 2
PLAYER_CHANCE_FLAG = 4
PLAYER_TERMINAL_FLAG = 8
PLAYER_FOLD_FLAG = 24

# Action::None < Fold < Check < Call < Bet < Raise < AllIn, the derived Ord that
# action_tree.rs:718-719 sorts by and remove_line's binary search relies on.
ACTION_RANK = {"fold": 1, "check": 2, "call": 3, "bet": 4, "raise": 5, "allin": 6}
WAGER_KINDS = ("bet", "raise", "allin")
TOKEN = {"fold": "F", "check": "X", "call": "C", "bet": "B", "raise": "R", "allin": "A"}
STREETS = ("flop", "turn", "river")
NEXT_STREET = {"flop": "turn", "turn": "river"}
REMAINING_STREETS = {"flop": 3, "turn": 2, "river": 1}
SIZE_PATTERN = re.compile(r"(?:\d{1,3}(?:\.\d{1,2})?)%|a")
LINE_PATTERN = re.compile(r"[FXC]|[BRA]\d{1,9}")
MAX_LINES = 4096


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def round_half_away(value: float) -> int:
    """Rust's `f64::round`: ties go away from zero, not to even as Python's `round` does."""
    require(math.isfinite(value), f"Nonfinite bet arithmetic: {value}")
    floor = math.floor(value)
    if value >= 0:
        return floor + 1 if value - floor >= 0.5 else floor
    ceil = math.ceil(value)
    return ceil - 1 if ceil - value >= 0.5 else ceil


def clamp(value: int, low: int, high: int) -> int:
    require(low <= high, f"Empty clamp range: {low} to {high}")
    return max(low, min(value, high))


def parse_sizes(text: str) -> list[tuple[str, float]]:
    """The subset of `BetSizeOptions` (bet_size.rs:85-115) a committed case can name.

    Percentages become `("pot", ratio)` and `a` becomes `("allin", 0.0)`, sorted the way
    `bet_size.rs:112-113` sorts them: by variant first, so every pot size precedes the all-in.
    An additive, geometric or previous-bet-relative size is rejected rather than guessed;
    `capture.py`'s `BET_PATTERN` and `RAISE_PATTERN` already refuse them in an input file.
    """
    require(type(text) is str, "A size menu must be a string")
    if text == "":
        return []
    sizes: list[tuple[str, float]] = []
    for item in text.split(","):
        item = item.strip()
        require(
            SIZE_PATTERN.fullmatch(item) is not None,
            f"Unsupported size for line derivation: {item!r}",
        )
        if item == "a":
            sizes.append(("allin", 0.0))
        else:
            sizes.append(("pot", float(item[:-1]) / 100.0))
    sizes.sort(key=lambda size: (size[0] == "allin", size[1]))
    return sizes


@dataclass(frozen=True)
class Config:
    """The `TreeConfig` fields that decide which actions exist (action_tree.rs:59-150)."""

    initial_street: str
    starting_pot: int
    effective_stack: int
    add_all_in_threshold: float
    force_all_in_threshold: float
    merging_threshold: float
    # street -> (bet sizes, raise sizes) per player, indexed by PLAYER_OOP / PLAYER_IP.
    bet_sizes: dict[str, tuple[tuple[list, list], tuple[list, list]]]


@dataclass(frozen=True)
class Info:
    """`BuildTreeInfo` (action_tree.rs:159-166)."""

    prev_action: tuple[str, int | None]
    num_bets: int
    allin_flag: bool
    stack: tuple[int, int]
    prev_amount: int

    @staticmethod
    def new(stack: int) -> Info:
        return Info(("none", None), 0, False, (stack, stack), 0)

    def create_next(self, player: int, action: tuple[str, int | None]) -> Info:
        kind, amount = action
        stack = list(self.stack)
        num_bets, allin_flag, prev_amount = (
            self.num_bets,
            self.allin_flag,
            self.prev_amount,
        )
        if kind == "call":
            num_bets = 0
            stack[player] = stack[player ^ 1]
            prev_amount = 0
        elif kind in WAGER_KINDS:
            to_call = stack[player] - stack[player ^ 1]
            num_bets += 1
            allin_flag = kind == "allin"
            stack[player] -= amount - prev_amount + to_call
            prev_amount = amount
        # Check, chance and fold leave every field but `prev_action` alone, and this module
        # never reads `oop_call_flag` because donk sizes are unsupported here.
        return replace(
            self,
            prev_action=action,
            num_bets=num_bets,
            allin_flag=allin_flag,
            stack=(stack[0], stack[1]),
            prev_amount=prev_amount,
        )


def config_from_case(case: dict) -> Config:
    """Read a `cases.json` case into the tree configuration, rejecting what is unsupported."""
    menus = case["menus"]
    require(
        case["street"] in STREETS and set(menus) == set(STREETS),
        "A case must name a street and all three menus",
    )
    require(not case["donk_option"], "Donk options are not modelled by this derivation")
    sizes = {}
    for street in STREETS:
        menu = menus[street]
        require(
            menu.get("oop_donk", "") == "" and menu.get("ip_donk", "") == "",
            "Donk sizes are not modelled by this derivation",
        )
        sizes[street] = (
            (parse_sizes(menu["oop_bet"]), parse_sizes(menu["oop_raise"])),
            (parse_sizes(menu["ip_bet"]), parse_sizes(menu["ip_raise"])),
        )
    return Config(
        initial_street=case["street"],
        starting_pot=case["starting_pot"],
        effective_stack=case["effective_stack"],
        add_all_in_threshold=float(case["add_all_in_threshold"]),
        force_all_in_threshold=float(case["force_all_in_threshold"]),
        merging_threshold=0.0,  # capture.mjs passes 0 to the binding.
        bet_sizes=sizes,
    )


def merge_bet_actions(
    actions: list[tuple[str, int | None]], pot: int, offset: int, param: float
) -> list[tuple[str, int | None]]:
    """`merge_bet_actions` (action_tree.rs:1067-1095). A no-op at the pinned param of 0."""
    eps = 1e-12
    current = 2**31 - 1
    result = []
    for action in reversed(actions):
        amount = action[1] if action[0] in WAGER_KINDS else -1
        if amount is not None and amount > 0:
            ratio = (amount - offset) / pot
            current_ratio = (current - offset) / pot
            threshold_ratio = (current_ratio - param) / (1.0 + param)
            if ratio < threshold_ratio * (1.0 - eps):
                result.append(action)
                current = amount
        else:
            result.append(action)
    result.reverse()
    return result


def push_actions(
    street: str, player: int, node_amount: int, info: Info, config: Config
) -> list[tuple[str, int | None]]:
    """`push_actions` (action_tree.rs:526-737), for the size kinds a case may name."""
    opponent = player ^ 1
    player_stack = info.stack[player]
    opponent_stack = info.stack[opponent]
    prev_amount = info.prev_amount
    to_call = player_stack - opponent_stack

    pot = config.starting_pot + 2 * (node_amount + to_call)
    max_amount = opponent_stack + prev_amount
    min_amount = clamp(prev_amount + to_call, 1, max_amount)
    bet_sizes, raise_sizes = config.bet_sizes[street][player]

    actions: list[tuple[str, int | None]] = []
    if info.prev_action[0] in ("none", "check", "chance"):
        actions.append(("check", None))
        for kind, ratio in bet_sizes:
            if kind == "allin":
                actions.append(("allin", max_amount))
            else:
                actions.append(("bet", round_half_away(pot * ratio)))
        if max_amount <= round_half_away(pot * config.add_all_in_threshold):
            actions.append(("allin", max_amount))
    else:
        actions.append(("fold", None))
        actions.append(("call", None))
        if not info.allin_flag:
            for kind, ratio in raise_sizes:
                if kind == "allin":
                    actions.append(("allin", max_amount))
                else:
                    actions.append(
                        ("raise", prev_amount + round_half_away(pot * ratio))
                    )
            if max_amount <= prev_amount + round_half_away(
                pot * config.add_all_in_threshold
            ):
                actions.append(("allin", max_amount))

    def is_above_threshold(amount: int) -> bool:
        new_pot = pot + 2 * (amount - prev_amount)
        threshold = round_half_away(new_pot * config.force_all_in_threshold)
        return max_amount <= amount + threshold

    clamped: list[tuple[str, int | None]] = []
    for kind, amount in actions:
        if kind in ("bet", "raise"):
            value = clamp(amount, min_amount, max_amount)
            if is_above_threshold(value):
                clamped.append(("allin", max_amount))
            else:
                clamped.append((kind, value))
        else:
            clamped.append((kind, amount))

    unique = sorted(set(clamped), key=lambda a: (ACTION_RANK[a[0]], a[1] or 0))
    return merge_bet_actions(unique, pot, prev_amount, config.merging_threshold)


def child_of(
    street: str,
    player: int,
    node_amount: int,
    info: Info,
    action: tuple[str, int | None],
) -> tuple[int, int]:
    """The child node's player field and matched amount (action_tree.rs:702-737)."""
    opponent = player ^ 1
    to_call = info.stack[player] - info.stack[opponent]
    after_call = (
        PLAYER_TERMINAL_FLAG if street == "river" else PLAYER_CHANCE_FLAG | player
    )
    after_check = opponent if player == PLAYER_OOP else after_call
    kind = action[0]
    if kind == "fold":
        return PLAYER_FOLD_FLAG | player, node_amount
    if kind == "check":
        return after_check, node_amount
    if kind == "call":
        return after_call, node_amount + to_call
    return opponent, node_amount + to_call


def line_token(action: tuple[str, int | None]) -> str:
    kind, amount = action
    if kind in WAGER_KINDS:
        require(type(amount) is int and amount > 0, f"Invalid wager amount: {amount}")
        return f"{TOKEN[kind]}{amount}"
    return TOKEN[kind]


def format_line(line: list[tuple[str, int | None]]) -> str:
    text = "-".join(line_token(action) for action in line)
    require(
        all(LINE_PATTERN.fullmatch(token) for token in text.split("-")),
        f"Derived line is not in the wrapper's grammar: {text}",
    )
    return text


def walk(case: dict, cap: int | None = None):
    """Yield `(street, wager_count, line, action, removed)` for every action in the tree.

    `removed` is True for the wagers the cap forbids; the walk does not descend into those,
    so the yielded lines are exactly the compact action tree the reference will be left with.
    """
    config = config_from_case(case)
    limit = case["max_raises"] if cap is None else cap
    require(type(limit) is int and 0 <= limit <= 32, f"Unsupported raise cap: {limit}")
    counted = 0

    def recurse(street, node_player, node_amount, info, line, wagers):
        nonlocal counted
        counted += 1
        require(counted <= 200_000, "Tree walk exceeded its node budget")
        if node_player & PLAYER_TERMINAL_FLAG:
            return
        if node_player & PLAYER_CHANCE_FLAG:
            next_street = NEXT_STREET[street]
            if not info.allin_flag:
                next_player = PLAYER_OOP
            elif street == "flop":
                next_player = PLAYER_CHANCE_FLAG | PLAYER_CHANCE
            else:
                next_player = PLAYER_TERMINAL_FLAG
            yield from recurse(
                next_street,
                next_player,
                node_amount,
                info.create_next(PLAYER_OOP, ("chance", None)),
                line,
                0,
            )
            return
        player = node_player
        for action in push_actions(street, player, node_amount, info, config):
            is_wager = action[0] in WAGER_KINDS
            removed = is_wager and info.num_bets >= limit + 1
            yield street, wagers + (1 if is_wager else 0), line + [
                action
            ], action, removed
            if removed:
                continue
            child_player, child_amount = child_of(
                street, player, node_amount, info, action
            )
            yield from recurse(
                street,
                child_player,
                child_amount,
                info.create_next(player, action),
                line + [action],
                wagers + (1 if is_wager else 0),
            )

    yield from recurse(
        config.initial_street,
        PLAYER_OOP,
        0,
        Info.new(config.effective_stack),
        [],
        0,
    )


def derive_removed_lines(case: dict, cap: int | None = None) -> list[str]:
    """The `removed_lines` entries that leave the reference with at most `max_raises` raises.

    Returns them in the order the walk finds them; no line is a prefix of another, so the
    order does not change the resulting tree. An empty list means the cap is already
    satisfied, which is what a cap the stack cannot reach gives.
    """
    lines = [format_line(line) for _, _, line, _, removed in walk(case, cap) if removed]
    require(
        len(lines) <= MAX_LINES, "Derived more removed lines than the wrapper accepts"
    )
    require(len(set(lines)) == len(lines), "Derived a duplicate removed line")
    tokens = [line.split("-") for line in lines]
    for index, line in enumerate(tokens):
        for other in tokens[:index] + tokens[index + 1 :]:
            require(
                line[: len(other)] != other,
                f"Derived line {'-'.join(line)} sits under another removed line",
            )
    return lines


def removed_lines_argument(lines: list[str]) -> str:
    """The single comma-separated string `GameManager::init` expects (lib.rs:179-180)."""
    require(
        all(type(line) is str and line and "," not in line for line in lines),
        "A removed line must be a non-empty comma-free string",
    )
    return ",".join(lines)


def max_street_wagers(case: dict, cap: int | None = None) -> int:
    """The most wagers any one street holds in the pruned tree. `cap + 1` when it is reached."""
    return max(
        (wagers for _, wagers, _, _, removed in walk(case, cap) if not removed),
        default=0,
    )
