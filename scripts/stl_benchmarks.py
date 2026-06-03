#!/usr/bin/env python3

import os
import argparse
import csv
from tabulate import tabulate

from run_bench import make_arg_parser, iter_bench


base_dir = '../resources/benchmarks/'
bench_names = {
    "cars": "car.stl",
    "thermostat": "thermostat.stl",
    "watertank": "watertank.stl",
    "railroad": "railroad.stl",
    "batteries": "battery.stl",
    "pcv": "pcv.stl",
    "mtl_requirements": "mtl.stl",
    "req_cps": "cps.stl",
    "avionics": "avionics.stl",
    "irrigation": "irrigation.stl",
}
bench_files = {name: os.path.join(base_dir, filename) for name, filename in bench_names.items()}


def check_benchmark(bench_name, bench_file, stltree_path, timeout, iters):
    argp = make_arg_parser()
    results = {'dataset': bench_name}

    # SMT BMC-like encoding
    args_smt = argp.parse_args(['--iters', str(iters), '--timeout', str(timeout), 'dummy.list', 'stlsat', '--engine', 'smt'])
    _, results['time_smt'], _, results['result_smt'] = iter_bench(bench_file, args_smt)

    # FOL encoding
    args_fol = argp.parse_args(['--iters', str(iters), '--timeout', str(timeout), 'dummy.list', 'stlsat', '--engine', 'fol'])
    _, results['time_fol'], _, results['result_fol'] = iter_bench(bench_file, args_fol)

    # STLTree tableau-based checking
    args_python_tableau = argp.parse_args(['--iters', str(iters), '--timeout', str(timeout), 'dummy.list', 'stltree', stltree_path])
    _, results['time_python_tableau'], _, results['result_python_tableau'] = iter_bench(bench_file, args_python_tableau)

    # Rust tableau-based checking
    args_rust_tableau = argp.parse_args(['--iters', str(iters), '--timeout', str(timeout), 'dummy.list', 'stlsat', '--engine', 'tableau'])
    _, results['time_rust_tableau'], _, results['result_rust_tableau'] = iter_bench(bench_file, args_rust_tableau)

    return results


# Print results
def pretty_print(results, timeout, csvfile):
    write_timeout = lambda t: 'TO' if t >= timeout else t

    # Table
    results_matrix = [
        [
            r['dataset'],
            write_timeout(r['time_smt']), r['result_smt'],
            write_timeout(r['time_fol']), r['result_fol'],
            write_timeout(r['time_python_tableau']), r['result_python_tableau'],
            write_timeout(r['time_rust_tableau']), r['result_rust_tableau'],
        ]
        for r in results
    ]

    # Table header
    header = ["Dataset", f"SMT (s)", "SMT Result", f"FOL (s)", "FOL Result", f"Python Tableau (s)", "Python Tableau Result", f"Rust Tableau (s)", "Rust Tableau Result"]

    print(tabulate(results_matrix, headers=header))

    if csvfile:
        with open(csvfile, 'w', newline='') as f:
            cw = csv.writer(f)
            cw.writerow(header)
            cw.writerows(results_matrix)


if __name__ == '__main__':
    argp = argparse.ArgumentParser()
    argp.add_argument('stltree_path', type=str, help='Path to the stltree executable')
    argp.add_argument('--timeout', type=int, default=120, help='Timeout for each benchmark (in seconds)')
    argp.add_argument('--iters', type=int, default=1, help='Number of iterations for each benchmark')
    argp.add_argument('--csv', type=str, default='', help='Path to output CSV file')
    args = argp.parse_args()

    results = [check_benchmark(name, file, args.stltree_path, args.timeout, args.iters) for name, file in bench_files.items()]

    print("Benchmark results:")
    pretty_print(results, args.timeout, args.csv)
