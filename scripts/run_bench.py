#!/usr/bin/env python3

import argparse
import platform
import os
from pathlib import Path
import time
import subprocess
import re
import statistics
import joblib
from tabulate import tabulate
import csv

time_pattern = re.compile(r"Total elapsed time \(s\): ([0-9]+\.[0-9]+)")
mem_pattern = re.compile(r"Max memory used \(KB\): ([0-9]+)")
result_pattern = re.compile(r"((sat)|(unsat)|(unknown)|(syntax error))")

if platform.system() == 'Darwin':
    time_bin = 'gtime'
else:
    time_bin = '/usr/bin/time'

def get_stlsat_args(args):
    stlsat_args = []
    if args.mltl:
        stlsat_args.append('--mltl')
    if args.no_memoization:
        stlsat_args.append('--no-memoization')
    if args.no_simple_first:
        stlsat_args.append('--no-simple-first')
    if args.no_formula_optimizations:
        stlsat_args.append('--no-formula-optimizations')
    if args.no_jump_rule:
        stlsat_args.append('--no-jump-rule')
    if args.no_formula_simplifications:
        stlsat_args.append('--no-formula-simplifications')
    if hasattr(args, 'engine'):
        stlsat_args.append('--engine')
        stlsat_args.append(args.engine)
    if hasattr(args, 'solver'):
        stlsat_args.append('--solver')
        stlsat_args.append(args.solver)

    return stlsat_args


def get_stltree_args(args):
    stltree_args = []
    if args.fol:
        stltree_args.append('--fol')
    if args.smt:
        stltree_args.append('--smt')
    if args.mltl:
        stltree_args.append('--mltl')
    if args.no_jump:
        stltree_args.append('--no-jump')
    if args.no_formula_optimizations:
        stltree_args.append('--no-formula-optimizations')
    if args.no_children_order_optimizations:
        stltree_args.append('--no-children-order-optimizations')
    if args.no_early_local_consistency_check:
        stltree_args.append('--no-early-local-consistency-check')
    if args.no_memoization:
        stltree_args.append('--no-memoization')
    if args.no_simple_nodes:
        stltree_args.append('--no-simple-nodes')
    if args.no_g_f:
        stltree_args.append('--no-g-f')

    return stltree_args


def caps_command(timeout, max_mem):
    if timeout > 0 or max_mem > 0:
        return [
            'systemd-run',
            '--quiet',
            '--user',
            '--scope',
            '-p',
            'KillSignal=SIGKILL',
            '-p',
            'MemoryMax={:d}M'.format(max_mem) if max_mem > 0 else 'MemoryMax=infinity',
            '-p',
            'MemorySwapMax=0' if max_mem > 0 else 'MemorySwapMax=infinity',
            '-p',
            'RuntimeMaxSec={:d}'.format(timeout) if timeout > 0 else 'RuntimeMaxSec=infinity'
        ]
    else:
        return []

def bench_command(fname, args):
    match args.tool:
        case 'stlsat':
            prog_path = os.path.join(Path(os.path.dirname(__file__)).parent.absolute(), 'target/release/stlsat')
            return [prog_path, '--smtlib-result'] + get_stlsat_args(args) + [fname]
        case 'stlsat-parallel':
            script_path = os.path.join(Path(os.path.dirname(__file__)).absolute(), 'parallel_sat.sh')
            return ['bash', script_path, fname, '--smtlib-result'] + get_stlsat_args(args)
        case 'stltree':
            return ['python3', args.stltree_path, '--smtlib-result'] + get_stltree_args(args) + [fname]
        case 'smt-quant':
            return ['bash', '-c', "'", args.translator_path, '-smtlib', f'"$(cat {fname})"', '|', args.z3_path, '-in', "'"]
    assert False

def exec_bench(fname, args):
    print('Evaluating file', fname, '...')

    command = ' '.join(
        caps_command(args.timeout, args.max_mem)
        + [
            time_bin,
            '-f',
            '"Total elapsed time (s): %e\nMax memory used (KB): %M"'
        ]
        + bench_command(fname, args)
    )

    if args.verbose >= 1:
        print(command)

    start_t = time.perf_counter() # to tentatively check timeout
    raw_res = subprocess.run(
        command,
        capture_output=True,
        shell=True
    )
    raw_stdout = raw_res.stdout.decode('utf-8')
    raw_stderr = raw_res.stderr.decode('utf-8')
    if args.verbose >= 1:
        print(raw_stdout)
    if args.verbose >= 2:
        print(raw_stderr)

    if raw_res.returncode != 0:
        if raw_res.returncode == -9:
            return (-1, -1, 'TO')
        elif raw_res.returncode == 137:
            if time.perf_counter() - start_t >= args.timeout:
                return (-1, -1, 'TO')
            else:
                return (-1, -1, 'OOM')
        return (-1, -1, 'Error {:d}'.format(raw_res.returncode))

    time_match = time_pattern.search(raw_stderr)
    mem_match = mem_pattern.search(raw_stderr)
    result_match = result_pattern.search(raw_stdout)
    if not result_match:
        result_match = result_pattern.search(raw_stderr)
    result = result_match[0] if result_match else 'no result!'
    return (
        float(time_match.group(1)),
        int(mem_match.group(1)),
        result
    )

def iter_bench(fname, args):
    get_column = lambda rows, i: [r[i] for r in rows]
    results = [exec_bench(fname, args) for _ in range(0, args.iters)]
    times = get_column(results, 0)
    mems = get_column(results, 1)
    res = get_column(results, 2)
    return (
        fname,
        statistics.mean(times),
        statistics.mean(mems),
        res[0],
    )

def exec_all(fnames, args):
    if args.jobs <= 1:
        return [list(iter_bench(fname, args)) for fname in fnames]
    else:
        results = joblib.Parallel(n_jobs=args.jobs)(joblib.delayed(iter_bench)(fname, args)
                                                    for fname in fnames)
        return [list(res) for res in results]

def expand_files(benchfile, base_path):
    files = []
    with open(benchfile, 'rt') as benchlist:
        for path in benchlist:
            path = path.strip()
            if base_path:
                path = os.path.join(base_path, path)
            if os.path.isfile(path):
                files.append(path)
    return files

def pretty_print(results, csvfile):
    header = ["Name", "Time (s)", "Total memory (KiB)", "Result"]

    print(tabulate(results, headers=header))

    if csvfile:
        with open(csvfile, 'w', newline='') as f:
            cw = csv.writer(f)
            cw.writerow(header)
            cw.writerows(results)


def make_arg_parser():
    argp = argparse.ArgumentParser()
    argp.add_argument('-i', '--iters', type=int, default=1, help='Number of executions for each benchmark')
    argp.add_argument('-j', '--jobs', type=int, default=1, help='Maximum number of benchmarks to execute in parallel')
    argp.add_argument('-t', '--timeout', type=int, default=0, help='Timeout in seconds for each benchmark. 0 = no timeout (default)')
    argp.add_argument('-M', '--max-mem', type=int, default=0, help='Maximum memory to be allocated in MiBs. 0 = no limit (default)')
    argp.add_argument('-v', '--verbose', action='count', default=0, help='Show individual benchmark results')
    argp.add_argument('--csv', type=str, default='', help='Output result in CSV format in the specified file')
    argp.add_argument('-b', '--base-path', type=str, default=None, help='Base path for benchmark files')
    argp.add_argument('benchmarks', type=str, help='File containing a list of banchmark files, one per line')
    subparsers = argp.add_subparsers(required=True, dest='tool')

    stlsat_p = subparsers.add_parser('stlsat', help='Use the Rust implementation of the tree-shaped tableau (stlsat)')
    stlsat_p.add_argument('--mltl', action='store_true', help='Use MLTL semantics for U and R operators.')
    stlsat_p.add_argument('--no-memoization', action='store_true', help='Disable memoization of tableau nodes.')
    stlsat_p.add_argument('--no-simple-first', action='store_true', help='Disable simple nodes optimization in tableau.')
    stlsat_p.add_argument('--no-formula-optimizations', action='store_true', help='Disable formula optimizations in tableau.')
    stlsat_p.add_argument('--no-jump-rule', action='store_true', help='Disable jump rule in tableau.')
    stlsat_p.add_argument('--no-formula-simplifications', action='store_true', help='Disable syntactic formula simplifications in tableau.')
    stlsat_p.add_argument('--engine', type=str, default='tableau', help='Choose satisfiability engine (default: tableau). Options: tableau, fol, smt.')
    stlsat_p.add_argument('--solver', type=str, default='z3', help='Change the solver for reals used by stlsat (default: z3). Options: auto, z3, dl.')

    stlsat_par_p = subparsers.add_parser('stlsat-parallel', help='Run stlsat with tableau and FOL encoding in parallel.')
    stlsat_par_p.add_argument('--mltl', action='store_true', help='Use MLTL semantics for U and R operators.')
    stlsat_par_p.add_argument('--no-memoization', action='store_true', help='Disable memoization of tableau nodes.')
    stlsat_par_p.add_argument('--no-simple-first', action='store_true', help='Disable simple nodes optimization in tableau.')
    stlsat_par_p.add_argument('--no-formula-optimizations', action='store_true', help='Disable formula optimizations in tableau.')
    stlsat_par_p.add_argument('--no-jump-rule', action='store_true', help='Disable jump rule in tableau.')
    stlsat_par_p.add_argument('--no-formula-simplifications', action='store_true', help='Disable syntactic formula simplifications in tableau.')

    stltree_p = subparsers.add_parser('stltree', help='Use the Python implementation of the tree-shaped tableau (stltree)')
    stltree_p.add_argument('stltree_path', type=str)
    stltree_p.add_argument('--fol', action='store_true', help='Use FOL satisfiability checker instead of tree-based tableau')
    stltree_p.add_argument('--smt', action='store_true', help='Use SMT-based bounded satisfiability checker instead of tree-based tableau')
    stltree_p.add_argument('--mltl', action='store_true', help='Use MLTL semantics for U and R operators.')
    stltree_p.add_argument('--no-jump', action='store_true', help='Disable jump rule in tableau.')
    stltree_p.add_argument('--no-formula-optimizations', action='store_true', help='Disable formula optimizations in tableau.')
    stltree_p.add_argument('--no-children-order-optimizations', action='store_true', help='Disable children order optimizations in tableau.')
    stltree_p.add_argument('--no-early-local-consistency-check', action='store_true', help='Perform local consistency checks on poised tableau nodes only.')
    stltree_p.add_argument('--no-memoization', action='store_true', help='Disable memoization of tableau nodes.')
    stltree_p.add_argument('--no-simple-nodes', action='store_true', help='Disable simple nodes optimization in tableau.')
    stltree_p.add_argument('--no-g-f', action='store_true', help='Do not use special rules for G and F in the tableau.')
    
    smt_quant_p = subparsers.add_parser('smt-quant', help='Use the SMT encoding with quantifiers and ILP')
    smt_quant_p.add_argument('translator_path', type=str)
    smt_quant_p.add_argument('z3_path', type=str, default='z3', nargs='?')

    return argp


if __name__ == '__main__':
    args = make_arg_parser().parse_args()

    print('Running benchmarks...')
    results = exec_all(expand_files(args.benchmarks, args.base_path), args)
    pretty_print(results, args.csv)
