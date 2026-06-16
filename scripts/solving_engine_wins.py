#!/usr/bin/env python3

import pprint
import os, sys, argparse
import pandas as pd

def valid_result(r):
    return r in {"sat", "unsat"}


def read_csv_files(tools, csv_files, timeout):
    data = {}
    for tool, csv_file in zip(tools, csv_files):
        if not os.path.exists(csv_file):
            print(f"Warning: {csv_file} does not exist. Skipping {tool}.")
            continue
        df = pd.read_csv(csv_file)
        bad = (df["Time (s)"] == -1) | (~df["Result"].map(valid_result))
        df.loc[bad, "Time (s)"] = timeout
        df.loc[bad, "Result"] = "timeout"
        data[tool] = df
    return data


def build_joint(tools, data, timeout):
    """Join one dataset's tools on their common instances and flag strict wins."""
    all_names = None
    for tool in tools:
        names = set(data[tool]["Name"])
        all_names = names if all_names is None else all_names & names

    joint = None
    for tool in tools:
        df = data[tool][data[tool]["Name"].isin(all_names)][["Name", "Time (s)", "Result"]].copy()
        df = df.rename(columns={"Time (s)": f"time_{tool}", "Result": f"result_{tool}"})
        joint = df if joint is None else joint.merge(df, on="Name")

    def consensus_result(row):
        results = {row[f"result_{t}"] for t in tools if valid_result(row[f"result_{t}"])}
        if len(results) == 1:
            return results.pop()
        if len(results) > 1:
            print(f"WARNING: inconsistent results for {row['Name']}: {results}")
        return "unknown"

    joint["verdict"] = joint.apply(consensus_result, axis=1)

    time_cols = [f"time_{t}" for t in tools]
    joint["min_time"] = joint[time_cols].min(axis=1)
    joint["n_at_min"] = joint[time_cols].eq(joint["min_time"], axis=0).sum(axis=1)

    for tool in tools:
        joint[f"win_{tool}"] = (
            (joint[f"time_{tool}"] < timeout) &
            (joint[f"time_{tool}"] == joint["min_time"]) &
            (joint["n_at_min"] == 1)
        )
    # draw = any instance that is not a strict win for exactly one tool;
    # this makes (sum of tool wins) + draw == n for every dataset, by construction.
    joint["draw"] = ~joint[[f"win_{t}" for t in tools]].any(axis=1)
    return joint


def dataset_block(tools, csvs, timeout, verdicts):
    """Return {verdict: {tool: wins, 'draw': d, 'n': n}} for one dataset."""
    data = read_csv_files(tools, csvs, timeout)
    present = [t for t in tools if t in data]
    empty = {**{t: 0 for t in tools}, "draw": 0, "n": 0}
    if not present:
        print("Warning: no CSVs found for this dataset; emitting zeros.")
        return {v: dict(empty) for v in verdicts}

    joint = build_joint(present, data, timeout)
    out = {}
    for v in verdicts:
        sv = joint[joint["verdict"] == v]
        block = {t: (int(sv[f"win_{t}"].sum()) if t in present else 0) for t in tools}
        block["draw"] = int(sv["draw"].sum())
        block["n"] = len(sv)
        out[v] = block
    return out


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Compact strict-wins table: one row per dataset, sat/unsat blocks.")
    parser.add_argument("--tools", required=True,
                        help="Comma-separated tool display names.")
    parser.add_argument("--timeout", type=float, default=120.0,
                        help="Timeout value in seconds (default: 120).")
    parser.add_argument("--dataset", nargs=2, action="append", metavar=("NAME", "CSVS"),
                        required=True,
                        help="A dataset row: its NAME and a comma-separated CSV list "
                             "(same order as --tools). Repeatable.")
    parser.add_argument("--verdicts", default="sat,unsat",
                        help="Comma-separated verdict blocks (default: sat,unsat). "
                             "Use e.g. 'sat' alone for a more compact table.")
    parser.add_argument("-o", "--output", default="wins_table.tex")
    parser.add_argument("--caption", default="Strict wins per benchmark.")
    parser.add_argument("--label", default="tab:wins")
    args = parser.parse_args()

    tools = [t.strip() for t in args.tools.split(",") if t.strip()]
    verdicts = [v.strip() for v in args.verdicts.split(",") if v.strip()]
    datasets = [name for name, _ in args.dataset]

    stats = {v: {} for v in verdicts}
    for name, csvs in args.dataset:
        csv_list = [c.strip() for c in csvs.split(",") if c.strip()]
        if len(csv_list) != len(tools):
            sys.exit(f"Error: dataset '{name}' has {len(csv_list)} CSVs for {len(tools)} tools.")
        block = dataset_block(tools, csv_list, args.timeout, verdicts)
        for v in verdicts:
            stats[v][name] = block[v]

    pprint.pprint(stats)