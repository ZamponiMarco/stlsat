"""Synthesize concrete signals from an stlsat satisfying trace.

The trace (from `--trace-extraction`) is a list of steps, one per integer
time unit; each step is a list of literal strings such as:

    on1            (!on1)           x > 0          (!x1 <= 21)
    d1 >= 1.4      x == 5           (x - y) > 0    |x| <= 3       true

For every variable we build a per-step value:
  * booleans   -> True / False / None (unconstrained)
  * numerics   -> a concrete value satisfying the step's constraints

Single-variable linear atoms (incl. |x + c| bounds) are solved directly.
Multi-variable atoms are handled by substituting already-fixed variables
(iterated); anything still unresolved is reported, not guessed.
"""

from __future__ import annotations

import re

_NUM = r"-?\d+(?:\.\d+)?"
_ID = r"[A-Za-z_]\w*"

_FLIP = {"<": ">", "<=": ">=", ">": "<", ">=": "<=", "==": "==", "!=": "!="}
_NEG = {"<": ">=", "<=": ">", ">": "<=", ">=": "<", "==": "!=", "!=": "=="}


# --------------------------------------------------------------- tokenization
def _tokenize(s: str) -> list[str]:
    return re.findall(rf"{_NUM}|{_ID}|<=|>=|==|!=|[-+*/<>|()]", s)


class _Unsupported(Exception):
    pass


class _Lin:
    """Linear expression: coeffs per variable + constant. May be wrapped in abs."""

    def __init__(self, coeffs=None, const=0.0, is_abs=False):
        self.coeffs = dict(coeffs or {})
        self.const = float(const)
        self.is_abs = is_abs

    def __add__(self, o):
        if self.is_abs or o.is_abs:
            raise _Unsupported("arith on abs")
        c = dict(self.coeffs)
        for v, k in o.coeffs.items():
            c[v] = c.get(v, 0.0) + k
        return _Lin(c, self.const + o.const)

    def __sub__(self, o):
        return self + o.scale(-1)

    def scale(self, k):
        if self.is_abs:
            raise _Unsupported("arith on abs")
        return _Lin({v: c * k for v, c in self.coeffs.items()}, self.const * k)

    def vars(self):
        return {v for v, c in self.coeffs.items() if c != 0}


class _Parser:
    def __init__(self, tokens):
        self.toks = tokens
        self.i = 0

    def peek(self):
        return self.toks[self.i] if self.i < len(self.toks) else None

    def next(self):
        t = self.peek()
        self.i += 1
        return t

    def parse_expr(self) -> _Lin:
        left = self.parse_term()
        while self.peek() in ("+", "-"):
            op = self.next()
            right = self.parse_term()
            left = left + right if op == "+" else left - right
        return left

    def parse_term(self) -> _Lin:
        left = self.parse_factor()
        while self.peek() in ("*", "/"):
            op = self.next()
            right = self.parse_factor()
            if op == "*":
                if not left.vars():
                    left = right.scale(left.const)
                elif not right.vars():
                    left = left.scale(right.const)
                else:
                    raise _Unsupported("nonlinear")
            else:
                if right.vars() or right.const == 0:
                    raise _Unsupported("division")
                left = left.scale(1.0 / right.const)
        return left

    def parse_factor(self) -> _Lin:
        t = self.next()
        if t == "-":
            return self.parse_factor().scale(-1)
        if t == "+":
            return self.parse_factor()
        if t == "(":
            e = self.parse_expr()
            if self.next() != ")":
                raise _Unsupported("paren")
            return e
        if t == "|":
            e = self.parse_expr()
            if self.next() != "|":
                raise _Unsupported("abs")
            e.is_abs = True
            return e
        if t is None:
            raise _Unsupported("eof")
        if re.fullmatch(_NUM, t):
            return _Lin({}, float(t))
        if re.fullmatch(_ID, t):
            return _Lin({t: 1.0})
        raise _Unsupported(t)


def _strip_neg(s: str):
    """Peel `(!...)` wrappers; return (inner, negated)."""
    s = s.strip()
    neg = False
    while True:
        if s.startswith("(!") and s.endswith(")"):
            s = s[2:-1].strip()
            neg = not neg
        elif s.startswith("!"):
            s = s[1:].strip()
            neg = not neg
        else:
            return s, neg


def parse_literal(s: str):
    """Return one of:
    ('bool', name, value)
    ('rel', lhs:_Lin, op, rhs_const, raw)   with rhs moved to a constant
    ('const', True/False)
    ('unsupported', raw)
    """
    raw = s
    s, neg = _strip_neg(s)
    if re.fullmatch(_ID, s):
        if s == "true":
            return ("const", not neg)
        if s == "false":
            return ("const", neg)
        return ("bool", s, not neg)

    m = re.search(r"(<=|>=|==|!=|<|>)", s)
    if not m:
        return ("unsupported", raw)
    op = m.group(1)
    try:
        lhs = _Parser(_tokenize(s[: m.start()])).parse_expr()
        rhs = _Parser(_tokenize(s[m.end():])).parse_expr()
    except _Unsupported:
        return ("unsupported", raw)
    if neg:
        op = _NEG[op]
    # move everything to the left: lhs' op const
    if rhs.is_abs and not lhs.is_abs:
        lhs, rhs, op = rhs, lhs, _FLIP[op]
    if rhs.vars():
        if lhs.is_abs:
            return ("unsupported", raw)
        lhs = lhs - rhs
        const = -lhs.const
        lhs.const = 0.0
    else:
        const = rhs.const - (0.0 if lhs.is_abs else lhs.const)
        if not lhs.is_abs:
            lhs.const = 0.0
    return ("rel", lhs, op, const, raw)


# ------------------------------------------------------------------- solving
class _Domain:
    """Feasible set for one variable in one step."""

    def __init__(self):
        self.lo = None      # (value, strict)
        self.hi = None
        self.eq = None
        self.exclude_points = []
        self.exclude_intervals = []   # (lo, hi, strict) from |x| >= c
        self.conflict = False

    def add(self, op, c):
        if op == "==":
            if self.eq is not None and self.eq != c:
                self.conflict = True
            self.eq = c
        elif op == "!=":
            self.exclude_points.append(c)
        elif op in ("<", "<="):
            strict = op == "<"
            if self.hi is None or c < self.hi[0] or (c == self.hi[0] and strict):
                self.hi = (c, strict)
        else:
            strict = op == ">"
            if self.lo is None or c > self.lo[0] or (c == self.lo[0] and strict):
                self.lo = (c, strict)

    def _ok(self, v):
        if self.eq is not None and v != self.eq:
            return False
        if self.lo is not None and (v < self.lo[0] or (self.lo[1] and v == self.lo[0])):
            return False
        if self.hi is not None and (v > self.hi[0] or (self.hi[1] and v == self.hi[0])):
            return False
        if any(abs(v - p) < 1e-9 for p in self.exclude_points):
            return False
        for lo, hi, strict in self.exclude_intervals:
            inside = (lo < v < hi) if strict else (lo <= v <= hi)
            if inside:
                return False
        return True

    def pick(self, prev=None):
        if self.conflict:
            return None
        if self.eq is not None:
            return self.eq if self._ok(self.eq) else None
        if prev is not None and self._ok(prev):
            return prev
        candidates = []
        lo = self.lo[0] if self.lo else None
        hi = self.hi[0] if self.hi else None
        if lo is not None and hi is not None:
            candidates += [(lo + hi) / 2, lo + (hi - lo) / 4, hi - (hi - lo) / 4]
        elif lo is not None:
            candidates += [lo + 1, lo + 0.5, lo + 2, (0 if lo < 0 else lo + 1)]
            if not self.lo[1]:
                candidates.insert(0, lo)
        elif hi is not None:
            candidates += [hi - 1, hi - 0.5, hi - 2, (0 if hi > 0 else hi - 1)]
            if not self.hi[1]:
                candidates.insert(0, hi)
        else:
            candidates += [0, 1, -1]
        # escape excluded intervals (|x| >= c) by jumping outside them
        for xlo, xhi, _ in self.exclude_intervals:
            candidates += [xhi + 1, xlo - 1]
        for p in self.exclude_points:
            candidates += [p + 1, p - 1]
        for c in candidates:
            if self._ok(c):
                return round(c, 6)
        return None


def _eval_lin(lin: _Lin, values):
    total = lin.const
    for v, k in lin.coeffs.items():
        if values.get(v) is None:
            return None
        total += k * values[v]
    return abs(total) if lin.is_abs else total


def _check(op, lhs_val, c):
    return {
        "<": lhs_val < c, "<=": lhs_val <= c + 1e-9,
        ">": lhs_val > c, ">=": lhs_val >= c - 1e-9,
        "==": abs(lhs_val - c) < 1e-9, "!=": abs(lhs_val - c) > 1e-9,
    }[op]


def synthesize(trace: list[list[str]]) -> dict:
    """Turn a trace (list of steps of literal strings) into concrete signals."""
    steps = len(trace)
    parsed = [[parse_literal(s) for s in step] for step in trace]

    bool_vars, num_vars = set(), set()
    for step in parsed:
        for p in step:
            if p[0] == "bool":
                bool_vars.add(p[1])
            elif p[0] == "rel":
                num_vars.update(p[1].vars())

    booleans = {v: [None] * steps for v in sorted(bool_vars)}
    numerics = {v: [None] * steps for v in sorted(num_vars)}
    unresolved = []
    prev_vals: dict = {}

    for t, step in enumerate(parsed):
        for p in step:
            if p[0] == "bool":
                booleans[p[1]][t] = p[2]

        rels = [p for p in step if p[0] == "rel"]
        values = {v: None for v in num_vars}
        pending = list(rels)

        for _ in range(4):  # substitution passes
            if not pending:
                break
            domains: dict = {}
            deferred = []
            for p in pending:
                _, lin, op, c, raw = p
                free = [v for v in lin.vars() if values.get(v) is None]
                if len(free) != 1:
                    deferred.append(p)
                    continue
                var = free[0]
                coef = lin.coeffs[var]
                rest = sum(lin.coeffs[v] * values[v] for v in lin.vars()
                           if v != var) if len(lin.vars()) > 1 else 0.0
                if lin.is_abs:
                    if len(lin.vars()) != 1 or abs(coef) != 1:
                        deferred.append(p)
                        continue
                    shift = lin.const  # |coef*var + const|
                    d = domains.setdefault(var, _Domain())
                    if op in ("<", "<="):
                        d.add(">" if op == "<" else ">=", (-c - shift) / coef)
                        d.add(op, (c - shift) / coef)
                    elif op in (">", ">="):
                        lo_x, hi_x = sorted([(-c - shift) / coef, (c - shift) / coef])
                        d.exclude_intervals.append((lo_x, hi_x, op == ">="))
                    else:
                        deferred.append(p)
                    continue
                # coef*var + rest  op  c
                new_c = (c - rest) / coef
                new_op = _FLIP[op] if coef < 0 else op
                domains.setdefault(var, _Domain()).add(new_op, new_c)
            for var, d in domains.items():
                v = d.pick(prev_vals.get(var))
                values[var] = v
                if v is None:
                    unresolved.append({"step": t, "atom": f"{var}: conflicting bounds"})
            pending = deferred
            if not domains:  # no progress
                break

        # verify everything we parsed; report what failed or stayed multi-var
        for p in rels:
            _, lin, op, c, raw = p
            val = _eval_lin(lin, values)
            if val is None or not _check(op, val, c):
                unresolved.append({"step": t, "atom": raw})

        for p in step:
            if p[0] == "unsupported":
                unresolved.append({"step": t, "atom": p[1]})
            elif p[0] == "const" and p[1] is False:
                unresolved.append({"step": t, "atom": "false"})

        for v in num_vars:
            if values[v] is None and v in prev_vals:
                values[v] = prev_vals[v]     # hold last value when unconstrained
            numerics[v][t] = values[v]
            if values[v] is not None:
                prev_vals[v] = values[v]

    # dedupe unresolved
    seen = set()
    uniq = [u for u in unresolved
            if (k := (u["step"], u["atom"])) not in seen and not seen.add(k)]

    return {
        "time": list(range(steps)),
        "numeric": numerics,
        "boolean": booleans,
        "unresolved": uniq,
    }
